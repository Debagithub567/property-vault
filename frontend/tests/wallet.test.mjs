import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { WalletReadyState } from '@solana/wallet-adapter-base';
import { PhantomWalletAdapter } from '@solana/wallet-adapter-phantom';
import { PublicKey, clusterApiUrl } from '@solana/web3.js';
import { createWalletSession, SOLANA_CLUSTER, SOLANA_RPC_ENDPOINT } from '../src/wallet/walletSession.js';

class FakePhantomAdapter extends EventEmitter {
  readyState = WalletReadyState.NotDetected;
  connectCalls = 0;
  disconnectCalls = 0;
  connectOperation = async () => {};

  async connect() {
    this.connectCalls++;
    await this.connectOperation();
  }

  async disconnect() {
    this.disconnectCalls++;
    this.emit('disconnect');
  }

  install() {
    this.readyState = WalletReadyState.Installed;
    this.emit('readyStateChange', this.readyState);
  }

  selectAccount(address) {
    this.emit('connect', { toBase58: () => address });
  }
}

test('app RPC is fixed to Devnet and missing Phantom is explained without calling the adapter', async () => {
  assert.equal(SOLANA_CLUSTER, 'devnet');
  assert.equal(SOLANA_RPC_ENDPOINT, clusterApiUrl('devnet'));
  const adapter = new FakePhantomAdapter();
  const session = createWalletSession(adapter);
  await session.connect();
  assert.equal(adapter.connectCalls, 0);
  assert.equal(session.getSnapshot().phase, 'disconnected');
  assert.match(session.getSnapshot().error, /Phantom was not detected/);
  adapter.install();
  assert.equal(session.getSnapshot().error, null);
});

test('connect, account switch, unexpected disconnect, and explicit disconnect update public key state', async () => {
  const adapter = new FakePhantomAdapter();
  adapter.install();
  const session = createWalletSession(adapter);
  let settleConnection;
  adapter.connectOperation = () => new Promise(resolve => { settleConnection = resolve; });
  const observed = [];
  const unsubscribe = session.subscribe(() => observed.push(session.getSnapshot().phase));
  const pending = session.connect();
  assert.equal(session.getSnapshot().phase, 'connecting');
  assert.equal(session.getSnapshot().address, null);
  adapter.selectAccount('FirstRealPublicKey11111111111111111111111111');
  settleConnection();
  await pending;
  assert.equal(session.getSnapshot().phase, 'connected');
  assert.equal(session.getSnapshot().address, 'FirstRealPublicKey11111111111111111111111111');
  adapter.selectAccount('SecondRealPublicKey2222222222222222222222222');
  assert.equal(session.getSnapshot().address, 'SecondRealPublicKey2222222222222222222222222');
  adapter.emit('disconnect');
  assert.equal(session.getSnapshot().phase, 'disconnected');
  assert.equal(session.getSnapshot().address, null);
  assert.match(session.getSnapshot().error, /Phantom disconnected/);
  adapter.connectOperation = async () => adapter.selectAccount('ThirdRealPublicKey33333333333333333333333333');
  await session.connect();
  await session.disconnect();
  assert.equal(session.getSnapshot().phase, 'disconnected');
  assert.equal(session.getSnapshot().address, null);
  assert.equal(session.getSnapshot().error, null);
  assert.equal(adapter.disconnectCalls, 1);
  assert.ok(observed.includes('connecting'));
  assert.ok(observed.includes('disconnecting'));
  unsubscribe();
});

test('rejection and malformed account changes never leave a stale connected address', async () => {
  const adapter = new FakePhantomAdapter();
  adapter.install();
  const session = createWalletSession(adapter);
  adapter.connectOperation = async () => {
    const error = new Error('User rejected the request');
    error.cause = { code: 4001 };
    throw error;
  };
  await session.connect();
  assert.equal(session.getSnapshot().phase, 'disconnected');
  assert.equal(session.getSnapshot().address, null);
  assert.match(session.getSnapshot().error, /rejected in Phantom/);

  adapter.connectOperation = async () => adapter.selectAccount('ValidRealPublicKey4444444444444444444444444');
  await session.connect();
  adapter.emit('error', { name: 'WalletPublicKeyError' });
  assert.equal(session.getSnapshot().address, null);
  assert.match(session.getSnapshot().error, /unavailable account/);
  assert.equal(adapter.disconnectCalls, 1);
});

test('the installed Phantom adapter forwards real provider account and disconnect events', async () => {
  const first = new PublicKey('11111111111111111111111111111111');
  const second = new PublicKey('Stake11111111111111111111111111111111111111');
  const provider = new EventEmitter();
  provider.isPhantom = true;
  provider.isConnected = false;
  provider.publicKey = first;
  provider.connect = async () => { provider.isConnected = true; };
  provider.disconnect = async () => { provider.isConnected = false; provider.emit('disconnect'); };
  const previous = {
    window: Object.getOwnPropertyDescriptor(globalThis, 'window'),
    document: Object.getOwnPropertyDescriptor(globalThis, 'document'),
    navigator: Object.getOwnPropertyDescriptor(globalThis, 'navigator'),
  };
  Object.defineProperty(globalThis, 'window', { configurable: true, value: { isPhantomInstalled: true, phantom: { solana: provider } } });
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { readyState: 'complete' } });
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { userAgent: 'Node test browser' } });
  try {
    const adapter = new PhantomWalletAdapter();
    const session = createWalletSession(adapter);
    assert.equal(session.getSnapshot().readyState, WalletReadyState.Installed);
    await session.connect();
    assert.equal(session.getSnapshot().address, first.toBase58());
    provider.publicKey = second;
    provider.emit('accountChanged', second);
    assert.equal(session.getSnapshot().address, second.toBase58());
    provider.emit('disconnect');
    assert.equal(session.getSnapshot().address, null);
    assert.match(session.getSnapshot().error, /Phantom disconnected/);
  } finally {
    for (const [key, descriptor] of Object.entries(previous)) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  }
});
