import { useEffect, useState, useSyncExternalStore } from 'react';
import { walletSession } from './walletSession';

const INSTALL_URL = 'https://phantom.com/download';

export function WalletControls() {
  const wallet = useSyncExternalStore(walletSession.subscribe, walletSession.getSnapshot);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const connected = wallet.phase === 'connected' && !!wallet.address;
  const busy = wallet.phase === 'connecting' || wallet.phase === 'disconnecting';
  const address = wallet.address;
  const shortAddress = address ? `${address.slice(0, 4)}…${address.slice(-4)}` : '';
  useEffect(() => setDetailsOpen(false), [address]);

  return (
    <div className="wallet-control">
      <button
        className="wallet cursor-interaction"
        type="button"
        disabled={busy}
        aria-expanded={connected ? detailsOpen && !wallet.error : undefined}
        aria-busy={busy}
        aria-label={connected ? `Phantom connected: ${address}. Show wallet options` : 'Connect Phantom wallet'}
        onClick={() => connected ? setDetailsOpen(open => !open) : void walletSession.connect()}
      >
        {wallet.phase === 'connecting' ? 'Connecting…' : wallet.phase === 'disconnecting' ? 'Disconnecting…' : connected ? shortAddress : 'Connect Phantom'}
      </button>
      {connected && detailsOpen && !wallet.error && (
        <div className="wallet-panel">
          <strong>Phantom connected</strong>
          <span className="wallet-address" title={address ?? undefined}>{address}</span>
          <small>App RPC: Solana Devnet. Your Phantom network setting is not verified here.</small>
          <button className="wallet-disconnect" type="button" onClick={() => { setDetailsOpen(false); void walletSession.disconnect(); }}>
            Disconnect Phantom
          </button>
        </div>
      )}
      {wallet.error && (
        <div className="wallet-panel wallet-error" role="alert">
          <span>{wallet.error}</span>
          {wallet.error.includes('not detected') && <a href={INSTALL_URL} target="_blank" rel="noopener noreferrer">Install Phantom ↗</a>}
        </div>
      )}
    </div>
  );
}
