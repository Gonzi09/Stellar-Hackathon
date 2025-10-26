// src/App.tsx

import { useState } from 'react';
import WalletConnect from './components/WalletConnect';
import ProjectCard from './components/ProjectCard';
import InvestmentModal from './components/InvestmentModal';
import './App.css';

function App() {
  const [walletAddress, setWalletAddress] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const handleConnect = (address: string) => {
    setWalletAddress(address);
    console.log('Wallet connected:', address);
  };

  const handleDisconnect = () => {
    setWalletAddress(null);
    console.log('Wallet disconnected');
  };

  const handleInvestSuccess = (txHash: string) => {
    setSuccessMessage(`Investment successful! Transaction: ${txHash}`);
    setTimeout(() => setSuccessMessage(null), 10000);
    
    // Reload project data
    window.location.reload();
  };

  return (
    <div className="app">
      <header className="app-header">
        <div className="container">
          <div className="header-content">
            <div className="logo">
              <h1>StellarBridge</h1>
              <p className="tagline">Micro-investments, Massive Impact</p>
            </div>
            <WalletConnect 
              onConnect={handleConnect}
              onDisconnect={handleDisconnect}
            />
          </div>
        </div>
      </header>

      <main className="main-content">
        <div className="container">
          {successMessage && (
            <div className="success-banner">
              <div className="success-content">
                <span className="success-icon">✅</span>
                <div className="success-text">
                  <strong>Investment Successful!</strong>
                  <p>
                    Transaction: {' '}
                    <a 
                      href={`https://stellar.expert/explorer/testnet/tx/${successMessage.split(': ')[1]}`}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      View on Stellar Expert
                    </a>
                  </p>
                </div>
              </div>
            </div>
          )}

          <section className="hero">
            <h2>Empower Entrepreneurs Worldwide</h2>
            <p>
              Invest as little as $1 to support micro-entrepreneurs building 
              better futures for their communities. Every investment is secured 
              on the Stellar blockchain.
            </p>
          </section>

          <section className="projects-section">
            <h3>Featured Projects</h3>
            <div className="projects-grid">
              <ProjectCard
                walletAddress={walletAddress}
                onInvest={() => setIsModalOpen(true)}
              />
            </div>
          </section>

          {walletAddress && (
            <InvestmentModal
              isOpen={isModalOpen}
              onClose={() => setIsModalOpen(false)}
              walletAddress={walletAddress}
              onSuccess={handleInvestSuccess}
            />
          )}
        </div>
      </main>

      <footer className="app-footer">
        <div className="container">
          <p>
            Built on <strong>Stellar</strong> • Powered by <strong>Soroban</strong>
          </p>
          <p className="network-badge">
            🔗 Connected to: <strong>Testnet</strong>
          </p>
        </div>
      </footer>
    </div>
  );
}

export default App;