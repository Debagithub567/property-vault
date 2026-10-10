import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { createIcons, icons } from 'lucide';
import shell from './components/dashboard-shell.html?raw';
import { initializePropertyStore } from './lib/propertyStore';
import { initializeCreatePropertyWizard } from './components/CreatePropertyWizard';
import { initializeInvestorDashboard } from './pages/InvestorDashboard';
import { initializeInvestmentFlow } from './components/InvestmentFlow';
import { initializeDashboardController } from './lib/dashboardController';
import { WalletControls } from './wallet/WalletControls';

/** Keep the approved HTML shell and React flows intact during source export. */
export default function App() {
  const initialized = useRef(false);
  const [walletSlot, setWalletSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    if (initialized.current) return;
    initialized.current = true;
    // Same icon library/version/API as the preview, now bundled locally.
    window.lucide = { createIcons: (options = {}) => createIcons({ icons, ...options }) };
    initializePropertyStore();
    initializeCreatePropertyWizard();
    initializeInvestorDashboard();
    initializeInvestmentFlow();
    initializeDashboardController();
    setWalletSlot(document.getElementById('pv-wallet-slot'));
  }, []);
  return <>
    <div dangerouslySetInnerHTML={{ __html: shell }} />
    {walletSlot && createPortal(<WalletControls />, walletSlot)}
  </>;
}
