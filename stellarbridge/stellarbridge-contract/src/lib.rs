#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, log, token, Address, BytesN, Env, Vec};

// ---------------------------
// Tipos y estructuras
// ---------------------------

#[contracttype]
#[derive(Clone, PartialEq)]
pub enum MilestoneStatus {
    Pending,
    EvidenceSubmitted,
    Verified,
    Rejected,
}

#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub amount: i128,
    pub deadline: u64,
    pub status: MilestoneStatus,
    pub evidence_hash: Option<BytesN<32>>,
}

#[contracttype]
#[derive(Clone)]
pub struct Project {
    pub id: u32,
    pub owner: Address,
    pub goal_amount: i128,
    pub raised: i128,
    pub milestones: Vec<Milestone>,
    pub active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Investment {
    pub investor: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    ProjectCounter,
    Project(u32),
    ProjectInvestments(u32),
    InvestorAmount(u32, Address),
    Verifier,
    Token,
}

// ---------------------------
// Contrato
// ---------------------------

#[contract]
pub struct StellarBridgeContract;

#[contractimpl]
impl StellarBridgeContract {
    /// Inicializa guardando `verifier` y `token`.
    /// Para SDK 22.x exigimos que el **verifier** firme la llamada.
    pub fn initialize(env: Env, verifier: Address, token: Address) {
        if env.storage().instance().has(&DataKey::Verifier) {
            panic!("Contract already initialized");
        }

        // Debe firmar el propio verificador:
        verifier.require_auth();

        env.storage().instance().set(&DataKey::Verifier, &verifier);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage()
            .instance()
            .set(&DataKey::ProjectCounter, &0u32);

        log!(&env, "Contract initialized");
    }

    pub fn create_project(
        env: Env,
        owner: Address,
        goal_amount: i128,
        milestone_amounts: Vec<i128>,
        milestone_deadlines: Vec<u64>,
    ) -> u32 {
        owner.require_auth();

        if milestone_amounts.len() != milestone_deadlines.len() {
            panic!("Milestone counts must match");
        }

        let mut milestones: Vec<Milestone> = Vec::new(&env);

        for i in 0..milestone_amounts.len() {
            let amount: i128 = milestone_amounts
                .get(i)
                .expect("amount index out of bounds");
            let deadline: u64 = milestone_deadlines
                .get(i)
                .expect("deadline index out of bounds");

            milestones.push_back(Milestone {
                amount,
                deadline,
                status: MilestoneStatus::Pending,
                evidence_hash: None,
            });
        }

        let mut counter: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ProjectCounter)
            .unwrap_or(0u32);
        counter += 1;

        let project = Project {
            id: counter,
            owner: owner.clone(),
            goal_amount,
            raised: 0,
            milestones,
            active: true,
        };

        env.storage()
            .instance()
            .set(&DataKey::Project(counter), &project);
        env.storage()
            .instance()
            .set(&DataKey::ProjectCounter, &counter);

        log!(&env, "Project created: {}", counter);
        counter
    }

    /// Recibe inversiones (token configurado) y las deja en escrow (cuenta del contrato).
    pub fn invest(env: Env, project_id: u32, investor: Address, amount: i128) {
        investor.require_auth();

        let mut project: Project = env
            .storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");

        if !project.active {
            panic!("Project not active");
        }
        if amount <= 0 {
            panic!("Amount must be positive");
        }

        let token_address: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");

        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&investor, &env.current_contract_address(), &amount);

        project.raised += amount;
        env.storage()
            .instance()
            .set(&DataKey::Project(project_id), &project);

        let investor_key = DataKey::InvestorAmount(project_id, investor.clone());
        let current: i128 = env
            .storage()
            .instance()
            .get(&investor_key)
            .unwrap_or(0i128);
        env.storage()
            .instance()
            .set(&investor_key, &(current + amount));

        let investments_key = DataKey::ProjectInvestments(project_id);
        let mut investments: Vec<Investment> = env
            .storage()
            .instance()
            .get(&investments_key)
            .unwrap_or(Vec::new(&env));

        investments.push_back(Investment {
            investor: investor.clone(),
            amount,
            timestamp: env.ledger().timestamp(),
        });

        env.storage()
            .instance()
            .set(&investments_key, &investments);

        log!(&env, "Investment received: {}", amount);
    }

    /// Owner sube el hash de evidencia para un hito pendiente.
    pub fn submit_evidence(
        env: Env,
        project_id: u32,
        milestone_index: u32,
        evidence_hash: BytesN<32>,
    ) {
        let mut project: Project = env
            .storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");

        // Solo el owner puede subir evidencia
        project.owner.require_auth();

        if milestone_index >= project.milestones.len() {
            panic!("Invalid milestone");
        }

        // Desempaquetar el hito, modificarlo y volver a guardarlo
        let mut milestone: Milestone = project
            .milestones
            .get(milestone_index)
            .expect("milestone index out of bounds");

        if milestone.status != MilestoneStatus::Pending {
            panic!("Milestone not pending");
        }

        milestone.evidence_hash = Some(evidence_hash);
        milestone.status = MilestoneStatus::EvidenceSubmitted;

        project.milestones.set(milestone_index, milestone);
        env.storage()
            .instance()
            .set(&DataKey::Project(project_id), &project);

        log!(&env, "Evidence submitted");
    }

    /// Verificador aprueba/rechaza; si aprueba, libera fondos al owner.
    pub fn verify_milestone(env: Env, project_id: u32, milestone_index: u32, approved: bool) {
        let verifier: Address = env
            .storage()
            .instance()
            .get(&DataKey::Verifier)
            .expect("Verifier not set");

        verifier.require_auth();

        let mut project: Project = env
            .storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");

        if milestone_index >= project.milestones.len() {
            panic!("Invalid milestone");
        }

        let mut milestone: Milestone = project
            .milestones
            .get(milestone_index)
            .expect("milestone index out of bounds");

        if milestone.status != MilestoneStatus::EvidenceSubmitted {
            panic!("No evidence");
        }

        if approved {
            milestone.status = MilestoneStatus::Verified;

            let token_address: Address = env
                .storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Token not set");

            let token_client = token::Client::new(&env, &token_address);
            token_client.transfer(
                &env.current_contract_address(),
                &project.owner,
                &milestone.amount,
            );

            log!(
                &env,
                "Milestone verified and funds released: {}",
                milestone.amount
            );
        } else {
            milestone.status = MilestoneStatus::Rejected;
            log!(&env, "Milestone rejected");
        }

        project.milestones.set(milestone_index, milestone);
        env.storage()
            .instance()
            .set(&DataKey::Project(project_id), &project);
    }

    // ---------------------------
    // Getters
    // ---------------------------

    pub fn get_project(env: Env, project_id: u32) -> Project {
        env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found")
    }

    pub fn get_investor_amount(env: Env, project_id: u32, investor: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::InvestorAmount(project_id, investor))
            .unwrap_or(0i128)
    }

    pub fn get_project_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ProjectCounter)
            .unwrap_or(0u32)
    }
}
