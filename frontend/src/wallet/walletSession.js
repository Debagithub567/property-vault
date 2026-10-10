import { WalletReadyState } from '@solana/wallet-adapter-base';
import { PhantomWalletAdapter } from '@solana/wallet-adapter-phantom';
import { clusterApiUrl } from '@solana/web3.js';

// This is the application's RPC selection. Phantom does not expose a reliable
// selected-cluster signal, so the UI must not describe this as the wallet network.
export const SOLANA_CLUSTER = 'devnet';
export const SOLANA_RPC_ENDPOINT = clusterApiUrl(SOLANA_CLUSTER);

function connectionMessage(error) {
  const message = error instanceof Error ? error.message : String(error ?? '');
  const cause = error?.cause;
  if (cause?.code === 4001 || /reject|denied|cancel/i.test(message)) {
    return 'Connection request was rejected in Phantom. Select Connect Phantom to try again.';
  }
  if (error?.name === 'WalletNotReadyError') {
    return 'Phantom was not detected. Install or enable Phantom, then refresh this page.';
  }
  if (error?.name === 'WalletDisconnectedError') {
    return 'Phantom disconnected. Connect again when you are ready.';
  }
  if (error?.name === 'WalletPublicKeyError') {
    return 'Phantom changed to an unavailable account. Reconnect to continue.';
  }
  if (error?.name === 'WalletDisconnectionError') {
    return 'Phantom reported a disconnect error. Check the extension before reconnecting.';
  }
  return 'Phantom reported a wallet error. Check the extension and try again.';
}

export function createWalletSession(adapter) {
  /** @type {{ phase: 'disconnected' | 'connecting' | 'connected' | 'disconnecting', address: string | null, readyState: import('@solana/wallet-adapter-base').WalletReadyState, error: string | null }} */
  let snapshot = {
    phase: 'disconnected',
    address: null,
    readyState: adapter.readyState,
    error: null,
  };
  const listeners = new Set();
  let manualDisconnect = false;
  const publish = patch => {
    snapshot = { ...snapshot, ...patch };
    listeners.forEach(listener => listener());
  };

  adapter.on('readyStateChange', readyState => {
    publish({
      readyState,
      error: readyState === WalletReadyState.Installed && snapshot.error?.includes('not detected')
        ? null
        : snapshot.error,
    });
  });
  adapter.on('connect', publicKey => {
    publish({ phase: 'connected', address: publicKey.toBase58(), error: null });
  });
  adapter.on('disconnect', () => {
    publish({
      phase: 'disconnected',
      address: null,
      error: manualDisconnect ? snapshot.error : snapshot.error || 'Phantom disconnected. Connect again when you are ready.',
    });
  });
  adapter.on('error', error => {
    if (error?.name === 'WalletPublicKeyError') {
      // The adapter cannot trust a malformed/null account-change payload.
      publish({ phase: 'disconnected', address: null, error: connectionMessage(error) });
      void Promise.resolve(adapter.disconnect()).catch(() => {});
    } else {
      publish({ error: connectionMessage(error) });
    }
  });

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getSnapshot: () => snapshot,
    async connect() {
      if (snapshot.phase === 'connecting' || snapshot.phase === 'connected') return;
      if (adapter.readyState !== WalletReadyState.Installed && adapter.readyState !== WalletReadyState.Loadable) {
        publish({ phase: 'disconnected', address: null, error: connectionMessage({ name: 'WalletNotReadyError' }) });
        return;
      }
      publish({ phase: 'connecting', address: null, error: null });
      try {
        await adapter.connect();
        // On iOS the adapter may redirect to Phantom instead of emitting connect.
        if (snapshot.phase === 'connecting') publish({ phase: 'disconnected' });
      } catch (error) {
        publish({ phase: 'disconnected', address: null, error: connectionMessage(error) });
      }
    },
    async disconnect() {
      if (snapshot.phase !== 'connected') return;
      manualDisconnect = true;
      publish({ phase: 'disconnecting', address: null, error: null });
      try {
        await adapter.disconnect();
      } catch (error) {
        publish({ error: connectionMessage(error) });
      } finally {
        manualDisconnect = false;
        publish({ phase: 'disconnected', address: null });
      }
    },
  };
}

export const walletSession = createWalletSession(new PhantomWalletAdapter());
