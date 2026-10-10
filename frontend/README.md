# PropertyVault frontend

Source export of the approved interactive PropertyVault prototype. The dashboard, owner views, three-step creation wizard, marketplace, three-step investment flow, investor dashboard, investment details, rent claims, withdrawal dialog, activity, analytics, settings and help experiences are preserved from the actual preview source.

## Stack and source fidelity

React 18.3.1, TypeScript, Vite and lucide 0.468.0. The published application was a hybrid of an HTML/imperative-JavaScript dashboard shell and React components written with `React.createElement`. This export retains those working JavaScript components and controller, with a TypeScript React bootstrap and documented demo types. It does **not** claim that the original components have all been rewritten or fully type-checked as TypeScript.

The original application uses scoped plain CSS, not Tailwind. No Tailwind or PostCSS configuration has been invented. Existing styles, responsive breakpoints, markup, photographs and interaction logic are retained. `SOURCE_MANIFEST.json` records original file hashes and the source revision.

Packaging adaptations: extract the existing HTML shell and inline CSS/controller into source files; wrap the original script initializers as module exports; replace React and lucide vendor/CDN loading with the same versions installed through npm; add Vite/TypeScript configuration. Initial hash routing also handles booting after the browser `load` event, which can happen with module loading. Stage 1 adds Phantom wallet connection only; no program transactions are wired.

## Prerequisites

- Node.js 20.19+ in the 20.x line, or Node.js 22.12+ (Node 24 LTS is recommended).
- npm and a modern browser with localStorage and BigInt support.
- Internet access for npm installation. Google Fonts is the sole remaining runtime external asset dependency.

## Install and run

Extract the archive into your existing repository so that `frontend/` sits alongside `Anchor.toml`, `programs/` and `tests/`.

```sh
cd property-vault/frontend
npm install
npm run dev
```

Open the URL printed by Vite, normally `http://127.0.0.1:5173`. For a reproducible install from the included lockfile, use `npm ci` instead of `npm install`.

```sh
npm run typecheck
npm test
npm run build
npm run preview
```

`build` runs the configured TypeScript check and creates `dist/`. `preview` serves that build, normally on port 4173. No lint script existed in the source and none has been introduced. The type check covers the new TypeScript files and project configuration; retained JavaScript uses `allowJs: true`, `checkJs: false`. The application needs no environment variables or credentials. The app RPC is fixed to Solana Devnet in `src/wallet/walletSession.js`.

## Stage 1: Phantom connection

The navbar's **Connect Phantom** button requests permission from the Phantom browser extension. Once connected, it shows the real abbreviated public key; opening it reveals the full key and **Disconnect Phantom**. The claim preview displays the same live account or says no Phantom wallet is connected. Connection is explicit on every page load; no key, seed phrase or connection token is saved by this app. The Phantom adapter handles account-change and disconnect events, while the app clears stale addresses and reports errors. A missing extension prompts an installation link to Phantom's official download page.

**App: Devnet** labels the application's configured RPC cluster. It does not verify RPC reachability or Phantom's selected network. `Anchor.toml` may still use localnet for Anchor CLI operations; it does not control the frontend. See [WALLET_STAGE1.md](WALLET_STAGE1.md) for manual browser checks.

## Structure

```text
frontend/
  index.html
  package.json / package-lock.json
  tsconfig.json / vite.config.ts
  public/assets/                  Approved property photographs
  src/
    main.tsx / App.tsx            Typed bootstrap; mounts approved source
    components/
      dashboard-shell.html       Exact dashboard/owner/navigation markup
      CreatePropertyWizard.js    Original three-step React listing flow
      InvestmentFlow.js          Original three-step React investment flow
    pages/InvestorDashboard.js   Portfolio, details, earnings and claims
    lib/
      propertyStore.js           Canonical demo property/investor ledger
      dashboardController.js     Original navigation, cards and dialogs
    wallet/                       Phantom adapter, state and navbar control
    types/                       Documented demo schema and icon boundary
    styles/                      Original scoped styles in original order
  tests/                         Export verification tests
  README.md / INTEGRATION_NOTES.md / VERIFICATION_REPORT.md
  SOURCE_MANIFEST.json
```

The existing React flows keep separate React roots. The root bootstrap deliberately does not enable StrictMode replay, which would duplicate the original one-time initializers. Navigation remains the approved hash/DOM navigation; React Router has not been substituted. Some navigation entries open dialogs rather than dedicated routed pages, as in the approved preview.

## Mock data and financial calculations

`src/lib/propertyStore.js` is the shared source of property terms, sold counts, wallet balance, holdings, historical contributions, finalized rental periods and purchase receipts. It also owns the original seed records and validates persisted records. Creation, investment and investor views consume this store. Existing display helpers and configuration checks remain in their original components to avoid a risky refactor.

USDC calculations use BigInt micro-units (one USDC = 1,000,000 units). Persisted amounts are strings so JSON serialization does not lose precision. Listing prices and estimated rent are decimal-USDC strings. Tokens held and historical investment contributions are tracked separately. The initial investor has seven tokens, 710 USDC of historical contributions, 25 USDC claimable and 20 USDC previously claimed; the initial mock wallet has 1,000 USDC. These historical contributions do not purport to equal today's listing token price multiplied by current balances.

Purchases validate supply, whole-token quantity and available mock funds. A single localStorage write commits balance, holdings, sold count and activity together. Request IDs prevent duplicate purchase commits. Claims are manual, one property-period at a time; completed demo claims credit the mock balance and update entitlement statuses. No transaction signatures or block explorer URLs are fabricated.

## Persistence and uploaded images

The canonical key is `propertyvault.shared-demo.v2`. Legacy keys `propertyvault.demo-listings.v1` and `propertyvault.investor.v1` are migrated when valid. Shared changes are observed by the existing subscribers; the original cross-tab storage listener is retained. Uploaded images are resized to a maximum dimension of 1200 pixels and stored as JPEG data URLs in demo listing records; the original upload size limit and preview behavior are retained.

localStorage is origin-specific. Your hosted preview's saved holdings or uploaded photographs will not automatically appear on localhost. This archive contains the approved source and bundled assets, not private browser storage from the hosted session. To reset only this demo, remove its three named keys in browser developer tools and reload. Storage quota or privacy settings may prevent saving large image collections; purchase error handling preserves the existing rollback behavior.

## Assets

`public/assets/skyline.jpg`, `harbor.jpg` and `downtown.jpg` are the exact photographs used in the approved source, derived from the user-provided reference. No replacement photographs were introduced. Separate photo license notices were not present in the source. React/lucide are bundled by Vite from npm dependencies; their package licenses remain in npm distributions. DM Sans retains the original Google Fonts CSS URL in `src/styles/dashboard.css`; font rendering uses the existing sans-serif fallback if that service is unavailable.

## Limitations and future integration

This remains a frontend prototype with a real Phantom connection and a Devnet RPC setting. Listing/tokenization, investment payments, tokens, capital withdrawals, rent claims and activity are simulated. The local demo balance and portfolio are independent of the connected public key and should not be read as on-chain balances. The existing owner overview and analytics include illustrative baseline figures; owner withdrawal and settings interactions retain their original in-memory/toast behavior rather than gaining new persistence. Seed timestamps remain the approved demo timestamps.

No Anchor client, RPC reads, backend, automatic payouts, Claim All, keepers or token trading has been added. Property tokens describe economic participation, not legal property title. Rental projections are illustrative; unsold-token handling is unresolved and this export does not establish a contract payout formula.

Use `INTEGRATION_NOTES.md` to plan incremental integration alongside your Anchor programs. Start at the shared ledger boundary and the existing action handlers. Actual instruction signatures, accounts, PDAs, token program choice, mint decimals and entitlement rules must be verified against your repository's program and IDL before implementing those adapters. This export does not modify or assume any Anchor account names.
