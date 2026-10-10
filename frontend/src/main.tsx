import { createRoot } from 'react-dom/client';
import App from './App';
// Preserve the published CSS order: flow styles first, dashboard styles last.
import './styles/wizard.css';
import './styles/investor.css';
import './styles/invest.css';
import './styles/dashboard.css';

const root = document.getElementById('root');
if (!root) throw new Error('Missing PropertyVault root element.');
// The approved application has one-time imperative initializers and separate
// React flow roots. StrictMode replay would mount those legacy roots twice.
createRoot(root).render(<App />);
