import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';

test('approved UI mounts, navigates, invests, opens details and claims a period', async () => {
  const shell = readFileSync(new URL('../src/components/dashboard-shell.html', import.meta.url), 'utf8');
  const dom = new JSDOM(`<!doctype html><html><body>${shell}</body></html>`, { url: 'http://localhost/#my-investments', pretendToBeVisual: true });
  const { window } = dom;
  // Stop the adapter's extension-detection poll in this unrelated UI test.
  window.isPhantomInstalled = true;
  window.phantom = { solana: { isPhantom: true } };
  // jsdom has no layout/scrolling engine; retain the real browser implementation.
  window.HTMLElement.prototype.scrollTo = function () {};
  await new Promise(resolve => window.addEventListener('load', resolve, { once: true }));
  for (const name of ['window', 'document', 'navigator', 'localStorage', 'history', 'location', 'CustomEvent', 'HTMLElement', 'Element', 'Node']) {
    Object.defineProperty(globalThis, name, { value: name === 'window' ? window : window[name], configurable: true, writable: true });
  }
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  const { act } = await import('react');
  const { createIcons, icons } = await import('lucide');
  window.lucide = globalThis.lucide = { createIcons: (options = {}) => createIcons({ icons, ...options }) };
  const modules = await Promise.all([
    import('../src/lib/propertyStore.js'),
    import('../src/components/CreatePropertyWizard.js'),
    import('../src/pages/InvestorDashboard.js'),
    import('../src/components/InvestmentFlow.js'),
    import('../src/lib/dashboardController.js'),
  ]);
  await act(async () => {
    modules[0].initializePropertyStore();
    modules[1].initializeCreatePropertyWizard();
    modules[2].initializeInvestorDashboard();
    modules[3].initializeInvestmentFlow();
    modules[4].initializeDashboardController();
  });
  const select = selector => {
    const element = document.querySelector(selector);
    assert.ok(element, `Expected ${selector}`);
    return element;
  };
  const button = (text, scope = document) => {
    const match = [...scope.querySelectorAll('button')].find(b => b.textContent.trim() === text);
    assert.ok(match, `Expected button ${text}`);
    return match;
  };
  const click = element => act(async () => element.click());
  const edit = (element, value) => act(async () => {
    const prototype = element.tagName === 'TEXTAREA' ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, 'value').set.call(element, value);
    element.dispatchEvent(new window.Event('input', { bubbles: true }));
  });
  const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
  assert.equal(select('#pv-investor').hidden, false, 'hash routing works after the load event');
  assert.equal(window.PVInvestor.totals().invested, 710000000n);
  assert.ok(document.querySelector('svg.lucide'), 'bundled lucide renders original icons');

  await click(select('[data-nav="Explore Properties"]'));
  assert.equal(select('#pv-investor').hidden, true);
  assert.equal(select('.properties .card-actions button[data-invest="harbor"]').disabled, true);
  await click(select('.properties .card-actions button[data-invest="downtown"]'));
  assert.match(select('.f-modal').textContent, /Investment Offering/);
  assert.match(select('.f-modal').textContent, /1,050 USDC/);
  assert.equal(document.querySelector('#f-quantity'), null, 'quantity is absent from step 1');
  await click(button('Continue to Investment', select('.f-modal')));
  assert.equal(select('#f-quantity').value, '1');
  await edit(select('#f-quantity'), '0');
  assert.equal(button('Review Investment', select('.f-modal')).disabled, true);
  await edit(select('#f-quantity'), '1');
  for (let i = 0; i < 4; i++) await click(select('[aria-label="Increase token quantity"]'));
  assert.equal(select('#f-quantity').value, '5');
  await click(select('[aria-label="Decrease token quantity"]'));
  assert.equal(select('#f-quantity').value, '4');
  await click(select('[aria-label="Increase token quantity"]'));
  assert.match(select('.f-modal').textContent, /105 USDC/);
  await click(button('Review Investment', select('.f-modal')));
  const confirm = button('Confirm Investment', select('.f-modal'));
  assert.equal(confirm.disabled, true);
  await click(select('.f-confirm input'));
  assert.equal(confirm.disabled, false);
  await act(async () => {
    confirm.click();
    confirm.click();
    await delay(850);
  });
  assert.match(select('.f-modal').textContent, /Demo Investment Complete/);
  assert.equal(window.PVStore.balance(), 895000000n);
  assert.equal(window.PVStore.snapshot().purchases.length, 1);
  await click(button('View My Investments', select('.f-modal')));
  assert.equal(select('#pv-investor').hidden, false);
  assert.equal(window.PVInvestor.totals().invested, 815000000n);
  assert.equal(window.PVInvestor.totals().tokens, 12);
  await click(select('[aria-label="View investment details for Downtown Mixed-Use"]'));
  assert.match(select('.i-dialog').textContent, /315 USDC/);
  await click(button('Claim available rent', select('.i-dialog')));
  assert.match(select('.i-dialog').textContent, /Downtown Mixed-Use/);
  assert.match(select('.i-dialog').textContent, /13 USDC/);
  await act(async () => {
    button('Simulate claim', select('.i-dialog')).click();
    await delay(700);
  });
  assert.equal(window.PVInvestor.totals().claimable, 12000000n);
  assert.equal(window.PVInvestor.totals().claimed, 33000000n);
  assert.equal(window.PVStore.balance(), 908000000n);
  assert.match(window.localStorage.getItem('propertyvault.shared-demo.v2'), /Demo claimed/);

  // Existing ancillary dialogs and owner actions remain available.
  await act(async () => document.dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))); 
  await click(select('[data-nav="Dashboard"]'));
  for (const action of ['analytics', 'settings', 'help', 'withdraw']) {
    const actionButton = document.querySelector(`[data-action="${action}"]`);
    if (!actionButton) continue;
    await click(actionButton);
    assert.equal(select('.overlay').hidden, false, `${action} opens its existing dialog`);
    await click(select('.dialog .dialog-close'));
  }
  await click(select('[data-action="list"]'));
  assert.ok(document.querySelector('#pv-wizard [role="dialog"]'), 'original listing wizard opens');
  await edit(select('#w-name'), 'Export Test Residence');
  await edit(select('#w-location'), 'Portland, OR');
  await edit(select('#w-description'), 'A local demo listing used to verify the exported owner flow.');
  await click(button('Continue →', select('#pv-wizard')));
  await edit(select('#w-price'), '12.5');
  await edit(select('#w-supply'), '40');
  await edit(select('#w-rent'), '500');
  await edit(select('#w-allocation'), '70');
  assert.match(select('#pv-wizard').textContent, /350 USDC/);
  await click(button('Continue →', select('#pv-wizard')));
  await click(select('.w-confirm input'));
  await act(async () => {
    button('Create & Tokenize Property', select('#pv-wizard')).click();
    await delay(900);
  });
  const listing = window.PVStore.getListings().find(p => p.name === 'Export Test Residence');
  assert.ok(listing);
  assert.equal(listing.price, '12.5');
  assert.equal(listing.supply, '40');
  assert.equal(listing.allocation, '70');
  await click(select('[data-nav="Explore Properties"]'));
  assert.ok(document.querySelector(`.card-actions button[data-invest="${listing.id}"]`), 'created listing automatically appears in marketplace');
  // This is DOM behavior verification, not browser visual/layout verification.
  await act(async () => { await delay(5100); });
  window.close();
});
