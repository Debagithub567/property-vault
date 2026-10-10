# Frontend export verification

Exported from the actual PropertyVault preview source at revision `2c6d401d40b0b96739d17ff4153f8c3b06dd3a73`. This is not a reconstruction from screenshots. Original source hashes and packaging adaptations are recorded in `SOURCE_MANIFEST.json`.

## Source included

- Dashboard/owner/sidebar/header shell and original navigation/dialog controller.
- Original React creation wizard, investment wizard and investor dashboard with details, earnings, claims and activity.
- Canonical shared demo ledger, validations, exact micro-USDC arithmetic and existing localStorage migration/persistence.
- All four original stylesheets and all three approved property photographs.
- React/TypeScript/Vite bootstrap, npm manifest/lockfile, configuration, ignore/environment example, tests, README and integration notes.

The shell markup, four CSS bodies, three image files and four original component/store bodies were compared against the publication source and match. Components/store differ only in module imports and initialization wrappers. The controller additionally handles initial hash routing when module initialization occurs after `load`. No screens, features or visual styles were rewritten or removed.

## Executed checks

Environment: Node.js 24.19.0; npm 11.9.0. Package manager: npm; one matching `package-lock.json`.

| Check | Result |
| --- | --- |
| `npm install` | Passed; pinned dependencies installed and lockfile generated. |
| `npm ci` | Passed; lockfile reinstall succeeded. |
| `npm run typecheck` | Passed. Checks new TypeScript bootstrap/configuration; preserved JavaScript is not checked with `checkJs`. |
| Lint | Not configured in the original or exported project; no lint command was claimed to run. |
| `npm test` | Passed: six tests, including ledger validation and a DOM interaction integration test. |
| `npm run build` | Passed; type check and Vite production compilation succeeded. |
| Clean archive extraction | Passed: extracted under a temporary directory, then repeated `npm ci`, type check, all six tests and production build. |
| Archive structure | Passed: single `frontend/` root; no dependency trees, compiled output, caches, hosting metadata, git data or credential files included. |

The interaction test mounts the actual exported React/DOM modules, verifies initial hash navigation, bundled icons, fully funded cards, all three investment steps, editable quantity/plus/minus calculations, required confirmation, duplicate-click prevention, success navigation, updated portfolio totals, investment detail data, a manual property-period claim, ancillary dialogs and owner property creation that appears in the marketplace. Ledger tests additionally verify reload persistence, fractional-USDC exactness, insufficient funds, funding limits and storage-failure rollback.

## Practical limits and remaining dependencies

- These checks include simulated DOM interaction testing, not visual testing in a full browser. Desktop/tablet/mobile CSS was preserved exactly; pixel rendering, image upload canvas decoding and touch behavior were not newly verified in a real browser during this export.
- Google Fonts / DM Sans uses the original remote CSS URL; its font files remain external. All property photos and runtime React/lucide code are local/bundled after installation.
- Vite emits a non-blocking chunk-size warning for the single approximately 546-kB minified application bundle, including the retained icon library. No behavior-changing optimization was introduced.
- npm emits environment/dependency notices (proxy environment configuration and jsdom's transitive `whatwg-encoding` deprecation); they did not prevent installation, testing or build.
- Saved browser session data is origin-specific and is not exported from the hosted preview. Localhost starts from the approved seeds unless existing valid local demo state is present there.
- Ancillary owner summary/analytics sample figures and in-memory withdrawal/settings behavior are retained exactly as prototype limitations, documented in the README. No real blockchain behavior, private credentials or fabricated transaction links are present.

Ready to open in VS Code as `frontend/` alongside the existing Anchor repository. The program/IDL was not modified or assumed; future integration action boundaries are documented in `INTEGRATION_NOTES.md`.
