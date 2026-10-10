# Stage 1 Phantom and Devnet browser checks

Automated tests cover missing wallet detection, pending connection, rejection, public-key state updates, account changes, disconnects and malformed account events using a controlled adapter. They also exercise the installed Phantom adapter against a stubbed browser provider. `npm run build` checks the production bundle. An actual Phantom extension, its approval popup and Solana Devnet network behavior require a browser and cannot be exercised by these Node tests.

With the frontend running on localhost in a browser:

1. Disable Phantom or open a clean browser profile. Select **Connect Phantom**. Confirm the missing-extension message and official **Install Phantom** link appear, with no fake address.
2. Enable Phantom and reload. Select **Connect Phantom**, then reject the request. Confirm a rejection message appears and the navbar remains disconnected. Retry and approve. Confirm the navbar abbreviation and expanded full public key match Phantom's selected account.
3. Switch accounts in Phantom. Confirm the navbar and the claim preview update to the new public key without a reload. If Phantom reports an unavailable account, confirm the previous key is cleared.
4. Disconnect from the navbar. Confirm the address disappears. Reconnect, then disconnect from Phantom itself. Confirm the address disappears and a reconnect message appears. Reload and confirm the app does not silently reconnect.
5. Confirm the navbar says **App: Devnet** and its tooltip explains this is the app RPC setting. This does not verify Phantom's network setting or RPC availability. Do not infer a network match from the label.
6. Open a listing, investment and claim preview. Confirm they still show demo data and complete only local simulations, with no Phantom transaction approval request and no claimed on-chain balance or signature.

No seed phrase, private key or secret is needed for these checks. The frontend uses `clusterApiUrl('devnet')`; the Anchor CLI's localnet provider setting is separate.
