#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec, Map, BytesN, token, log};

// Milestone status
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum MilestoneStatus {
    Pending,
    EvidenceSubmitted,
    Verified,
    Rejected,
}

// Individual milestone
#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub amount: i128,
    pub deadline: u64,
    pub status: MilestoneStatus,
    pub evidence_hash: Option<BytesN<32>>,
}

// Project structure
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

// Investment record
#[contracttype]
#[derive(Clone)]
pub struct Investment {
    pub investor: Address,
    pub amount: i128,
    pub timestamp: u64,
}

// Storage keys
#[contracttype]
pub enum DataKey {
    ProjectCounter,
    Project(u32),
    ProjectInvestments(u32),
    InvestorAmount(u32, Address),
    Verifier,
    Token,
}

#[contract]
pub struct StellarBridgeContract;

#[contractimpl]
impl StellarBridgeContract {
    
    /// Initialize contract with verifier and token
    pub fn initialize(env: Env, verifier: Address, token: Address) {
        if env.storage().instance().has(&DataKey::Verifier) {
            panic!("Contract already initialized");
        }
        
        verifier.require_auth();
        
        env.storage().instance().set(&DataKey::Verifier, &verifier);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::ProjectCounter, &0u32);
        
        log!(&env, "Contract initialized with verifier: {}", verifier);
    }
    
    /// Create a new project with milestones
    pub fn create_project(
        env: Env,
        owner: Address,
        goal_amount: i128,
        milestone_amounts: Vec<i128>,
        milestone_deadlines: Vec<u64>,
    ) -> u32 {
        owner.require_auth();
        
        if milestone_amounts.len() != milestone_deadlines.len() {
            panic!("Milestone amounts and deadlines must match");
        }
        
        let mut total_milestone_amount: i128 = 0;
        let mut milestones: Vec<Milestone> = Vec::new(&env);
        
        for i in 0..milestone_amounts.len() {
            let amount = milestone_amounts.get(i).unwrap();
            let deadline = milestone_deadlines.get(i).unwrap();
            
            total_milestone_amount += amount;
            
            milestones.push_back(Milestone {
                amount,
                deadline,
                status: MilestoneStatus::Pending,
                evidence_hash: None,
            });
        }
        
        if total_milestone_amount > goal_amount {
            panic!("Total milestone amount exceeds goal");
        }
        
        let mut counter: u32 = env.storage().instance().get(&DataKey::ProjectCounter).unwrap_or(0);
        counter += 1;
        
        let project = Project {
            id: counter,
            owner: owner.clone(),
            goal_amount,
            raised: 0,
            milestones,
            active: true,
        };
        
        env.storage().instance().set(&DataKey::Project(counter), &project);
        env.storage().instance().set(&DataKey::ProjectCounter, &counter);
        
        log!(&env, "Project {} created by {}", counter, owner);
        
        counter
    }
    
    /// Invest in a project
    pub fn invest(env: Env, project_id: u32, investor: Address, amount: i128) {
        investor.require_auth();
        
        let mut project: Project = env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");
        
        if !project.active {
            panic!("Project is not active");
        }
        
        if amount <= 0 {
            panic!("Investment amount must be positive");
        }
        
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        
        // Transfer tokens to contract
        let token_client = token::Client::new(&env, &token_address);
        token_client.transfer(&investor, &env.current_contract_address(), &amount);
        
        // Update project raised amount
        project.raised += amount;
        env.storage().instance().set(&DataKey::Project(project_id), &project);
        
        // Track investor contribution
        let investor_key = DataKey::InvestorAmount(project_id, investor.clone());
        let current: i128 = env.storage().instance().get(&investor_key).unwrap_or(0);
        env.storage().instance().set(&investor_key, &(current + amount));
        
        // Add to investments list
        let investments_key = DataKey::ProjectInvestments(project_id);
        let mut investments: Vec<Investment> = env.storage()
            .instance()
            .get(&investments_key)
            .unwrap_or(Vec::new(&env));
        
        investments.push_back(Investment {
            investor: investor.clone(),
            amount,
            timestamp: env.ledger().timestamp(),
        });
        
        env.storage().instance().set(&investments_key, &investments);
        
        log!(&env, "Investment of {} in project {} from {}", amount, project_id, investor);
    }
    
    /// Submit evidence for a milestone
    pub fn submit_evidence(
        env: Env,
        project_id: u32,
        milestone_index: u32,
        evidence_hash: BytesN<32>,
    ) {
        let mut project: Project = env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");
        
        project.owner.require_auth();
        
        if milestone_index >= project.milestones.len() {
            panic!("Invalid milestone index");
        }
        
        let mut milestone = project.milestones.get(milestone_index).unwrap();
        
        if milestone.status != MilestoneStatus::Pending {
            panic!("Milestone not in pending state");
        }
        
        milestone.evidence_hash = Some(evidence_hash.clone());
        milestone.status = MilestoneStatus::EvidenceSubmitted;
        
        project.milestones.set(milestone_index, milestone);
        env.storage().instance().set(&DataKey::Project(project_id), &project);
        
        log!(&env, "Evidence submitted for project {} milestone {}", project_id, milestone_index);
    }
    
    /// Verify a milestone (verifier only)
    pub fn verify_milestone(
        env: Env,
        project_id: u32,
        milestone_index: u32,
        approved: bool,
    ) {
        let verifier: Address = env.storage()
            .instance()
            .get(&DataKey::Verifier)
            .expect("Verifier not set");
        
        verifier.require_auth();
        
        let mut project: Project = env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");
        
        if milestone_index >= project.milestones.len() {
            panic!("Invalid milestone index");
        }
        
        let mut milestone = project.milestones.get(milestone_index).unwrap();
        
        if milestone.status != MilestoneStatus::EvidenceSubmitted {
            panic!("No evidence submitted for this milestone");
        }
        
        if approved {
            milestone.status = MilestoneStatus::Verified;
            
            // Release funds to project owner
            let token_address: Address = env.storage()
                .instance()
                .get(&DataKey::Token)
                .expect("Token not set");
            
            let token_client = token::Client::new(&env, &token_address);
            token_client.transfer(
                &env.current_contract_address(),
                &project.owner,
                &milestone.amount
            );
            
            log!(&env, "Milestone {} verified for project {}", milestone_index, project_id);
        } else {
            milestone.status = MilestoneStatus::Rejected;
            log!(&env, "Milestone {} rejected for project {}", milestone_index, project_id);
        }
        
        project.milestones.set(milestone_index, milestone);
        env.storage().instance().set(&DataKey::Project(project_id), &project);
    }
    
    /// Trigger refund if milestone deadline expired
    pub fn trigger_refund(env: Env, project_id: u32, milestone_index: u32) {
        let mut project: Project = env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found");
        
        if milestone_index >= project.milestones.len() {
            panic!("Invalid milestone index");
        }
        
        let milestone = project.milestones.get(milestone_index).unwrap();
        
        if env.ledger().timestamp() < milestone.deadline {
            panic!("Milestone deadline not yet expired");
        }
        
        if milestone.status == MilestoneStatus::Verified {
            panic!("Milestone already verified");
        }
        
        // Calculate refund amount (proportional to unverified milestones)
        let mut unverified_amount: i128 = 0;
        for i in milestone_index..project.milestones.len() {
            let m = project.milestones.get(i).unwrap();
            if m.status != MilestoneStatus::Verified {
                unverified_amount += m.amount;
            }
        }
        
        // Refund proportionally to investors
        let investments_key = DataKey::ProjectInvestments(project_id);
        let investments: Vec<Investment> = env.storage()
            .instance()
            .get(&investments_key)
            .unwrap_or(Vec::new(&env));
        
        let token_address: Address = env.storage()
            .instance()
            .get(&DataKey::Token)
            .expect("Token not set");
        
        let token_client = token::Client::new(&env, &token_address);
        
        for i in 0..investments.len() {
            let investment = investments.get(i).unwrap();
            let refund = (investment.amount * unverified_amount) / project.raised;
            
            if refund > 0 {
                token_client.transfer(
                    &env.current_contract_address(),
                    &investment.investor,
                    &refund
                );
            }
        }
        
        project.active = false;
        env.storage().instance().set(&DataKey::Project(project_id), &project);
        
        log!(&env, "Refund triggered for project {}", project_id);
    }
    
    /// Get project details
    pub fn get_project(env: Env, project_id: u32) -> Project {
        env.storage()
            .instance()
            .get(&DataKey::Project(project_id))
            .expect("Project not found")
    }
    
    /// Get investor amount for a project
    pub fn get_investor_amount(env: Env, project_id: u32, investor: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::InvestorAmount(project_id, investor))
            .unwrap_or(0)
    }
    
    /// Get total number of projects
    pub fn get_project_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::ProjectCounter)
            .unwrap_or(0)
    }
}