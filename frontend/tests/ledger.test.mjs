import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

function ledger(saved = new Map()) {
  const context = {
    localStorage: { getItem: key => saved.get(key) ?? null, setItem: (key, value) => saved.set(key, value) },
    window: { addEventListener() {}, dispatchEvent() {}, PVDemoListings: { amount: value => `${Number(value) / 1e6} USDC` } },
    CustomEvent: class { constructor(type) { this.type = type; } },
  };
  vm.createContext(context);
  const source = readFileSync(new URL('../src/lib/propertyStore.js', import.meta.url), 'utf8');
  vm.runInContext(source.replace('export function', 'function') + '\ninitializePropertyStore();', context);
  return { store: context.window.PVStore, saved, context };
}

test('purchase synchronizes exact cost, balance, holdings, supply, history and reload', () => {
  const { store, saved } = ledger();
  assert.equal(store.balance(), 1000000000n);
  const receipt = store.purchase('downtown', '5', 'local-test-purchase');
  assert.equal(receipt.amount, '105000000');
  assert.equal(store.balance(), 895000000n);
  assert.equal(store.property('downtown').sold, 92);
  const holding = store.investor().holdings.find(p => p.id === 'downtown');
  assert.equal(holding.tokens, 7);
  assert.equal(holding.invested, '315000000');
  assert.equal(store.investor().history[0].title, 'Purchased 5 property tokens');
  store.purchase('downtown', '5', 'local-test-purchase');
  assert.equal(store.balance(), 895000000n);
  assert.equal(store.snapshot().purchases.length, 1);
  assert.equal(ledger(saved).store.property('downtown').sold, 92);
});

test('quantity, fully funded supply and insufficient funds are enforced', () => {
  const { store } = ledger();
  for (const quantity of ['0', '-1', '1.5', '1e2', '', '64']) {
    assert.ok(store.investmentCheck('downtown', quantity).error);
  }
  assert.ok(store.investmentCheck('harbor', '1').error.includes('funded'));
  assert.ok(store.investmentCheck('downtown', '48').error.includes('insufficient'));
  assert.equal(store.balance(), 1000000000n);
});

test('new listings preserve per-property terms, exact decimal prices and funding limits', () => {
  const { store } = ledger();
  store.createListing({ ...store.property('skyline'), id: 'created-demo', name: 'Created demo', price: '0.125', supply: '10', rent: '75.25', allocation: '61' });
  assert.equal(store.getListings()[0].id, 'created-demo');
  assert.equal(store.property('created-demo').sold, 0);
  store.purchase('created-demo', '10', 'new-property-request');
  assert.equal(store.balance(), 998750000n);
  assert.equal(store.investor().holdings.find(p => p.id === 'created-demo').invested, '1250000');
  assert.ok(store.investmentCheck('created-demo', '1').error.includes('funded'));
});

test('claiming one finalized period credits once and leaves other periods unchanged', () => {
  const { store } = ledger();
  const investor = store.investor();
  investor.periods.find(p => p.id === 'skyline-2').claimed = true;
  store.setInvestor(investor);
  assert.equal(store.balance(), 1012000000n);
  assert.equal(store.investor().periods.find(p => p.id === 'downtown-2').claimed, false);
  store.setInvestor(investor);
  assert.equal(store.balance(), 1012000000n);
});

test('failed localStorage commit does not debit funds or mutate holdings', () => {
  const { store, context } = ledger();
  context.localStorage.setItem = () => { throw new Error('Storage quota exceeded'); };
  assert.throws(() => store.purchase('downtown', '5', 'failed-request'), /Storage quota/);
  assert.equal(store.balance(), 1000000000n);
  assert.equal(store.property('downtown').sold, 87);
  assert.equal(store.snapshot().purchases.length, 0);
});
