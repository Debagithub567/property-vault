# PropertyVault Devnet functional test results

Run date: 11 October 2026. Network: Solana Devnet, using the configured Chainstack RPC in `frontend/.env`.

```bash
node --import tsx ./node_modules/mocha/bin/mocha.js --timeout 120000 tests/devnet.test.ts
```

**Final clean run: 14 passing, 0 failing (about 3 minutes).** Thirteen tests submitted real transactions: nine succeeded and four were confirmed rejected by the intended contract error. The remaining test was a read-only deployment and compatibility check. No result below is a mock or a local-validator result.

The final run used isolated Property PDA [`EahyPRC2sPhR9m6VaLqG2Dfp5Cuo7kk9MmHHAFuevwF6`](https://explorer.solana.com/address/EahyPRC2sPhR9m6VaLqG2Dfp5Cuo7kk9MmHHAFuevwF6?cluster=devnet), property ID `9451116106879098133`. It listed 10 tokens at 1 mock USDC each, allocated 80% of collected rent to investors, and used a 180-second rental period. The owner wallet also served as the designated tenant for this isolated test. A fresh, in-memory investor wallet was used; the suite never printed its secret key. The suite minted only controlled mock USDC and transferred 0.02 Devnet SOL to that test wallet.

| Test | Outcome | Confirmed Devnet transaction |
| --- | --- | --- |
| Deployed ABI, existing account layouts, funding authority | Passed, read-only | None |
| Fund isolated investor and tenant | Passed | [47RmrdGvRVBmZE2THiJyFAimBiW6M8i7CYSWFj1tNsNmH36WMrjpKDQy6orTtUnvySmfwQSJ8DUZR8bfBFEjniuz](https://explorer.solana.com/tx/47RmrdGvRVBmZE2THiJyFAimBiW6M8i7CYSWFj1tNsNmH36WMrjpKDQy6orTtUnvySmfwQSJ8DUZR8bfBFEjniuz?cluster=devnet) |
| Create property and initialize Transfer Hook metadata atomically | Passed | [RAJvtUG5Fg1rfnYV8FAVA7hm19Trv5V84J2AU79NBpHit6efB3jTnu8UL886JSJT3rPEPu2EUzaxKmuf6ViwXEX](https://explorer.solana.com/tx/RAJvtUG5Fg1rfnYV8FAVA7hm19Trv5V84J2AU79NBpHit6efB3jTnu8UL886JSJT3rPEPu2EUzaxKmuf6ViwXEX?cluster=devnet) |
| Register investor and freeze Token-2022 ATA | Passed | [2smAvfwPGDJJi3FyWb8Da7DtjCYy7JZYzZAWRkushqUCFCWWUJZ2JqUHKnYbPsWZYgnwWrMsFGf8kfCL6ygD1u5m](https://explorer.solana.com/tx/2smAvfwPGDJJi3FyWb8Da7DtjCYy7JZYzZAWRkushqUCFCWWUJZ2JqUHKnYbPsWZYgnwWrMsFGf8kfCL6ygD1u5m?cluster=devnet) |
| Reject incorrect accepted terms hash | Passed; confirmed `StaleTerms`; state unchanged | [4o79SfQKEWrtcfzyzDGRfJE1BvsEtH5ST65FgQTMeYnap4AqVrCDZuhzqZUHyjcAVYxRpgRXg5KJqQHbmH1tNeLL](https://explorer.solana.com/tx/4o79SfQKEWrtcfzyzDGRfJE1BvsEtH5ST65FgQTMeYnap4AqVrCDZuhzqZUHyjcAVYxRpgRXg5KJqQHbmH1tNeLL?cluster=devnet) |
| Reject incorrect investor allocation | Passed; confirmed `StaleInvestorAllocation`; state unchanged | [2YMkWakB4oJ8Q6rWEAhxFsQsQenhvUzHc6fLrYw1ykALfHsw2q5ij6nu9iWHaHsGaG3EUSDFH9ShC7zvS3GE6SRa](https://explorer.solana.com/tx/2YMkWakB4oJ8Q6rWEAhxFsQsQenhvUzHc6fLrYw1ykALfHsw2q5ij6nu9iWHaHsGaG3EUSDFH9ShC7zvS3GE6SRa?cluster=devnet) |
| Invest in one token | Passed; 1 mock USDC moved to investment vault; one token moved to investor | [4MQBx6KPAVjer3VDtAk2o34tQR8SUKfFHSMEEw8P6REBoDDwZSXtbjycrwbT93324YzD7faPhx85fRnZcDjdNqEA](https://explorer.solana.com/tx/4MQBx6KPAVjer3VDtAk2o34tQR8SUKfFHSMEEw8P6REBoDDwZSXtbjycrwbT93324YzD7faPhx85fRnZcDjdNqEA?cluster=devnet) |
| Reject unauthorized capital withdrawal | Passed; confirmed `Unauthorized`; accounting unchanged | [3L5gMv3inoP5ayJCBbMbVp1L6SUJidZ5NLLEEsHeqagSFHpaa1muxHMsz6D6YpccAUxmWiwRJPqFMdYUEBm649WD](https://explorer.solana.com/tx/3L5gMv3inoP5ayJCBbMbVp1L6SUJidZ5NLLEEsHeqagSFHpaa1muxHMsz6D6YpccAUxmWiwRJPqFMdYUEBm649WD?cluster=devnet) |
| Owner withdraws raised capital | Passed; 1 mock USDC reached owner; vault and `total_withdrawn` updated | [34URVpdWmsNNbFE5d9b5CqZWgEfucjqjitgzGdZ1KvTd33YAgJpTGgSrqJjfMqrLe86X1YgKe2rZLgD3ipQ6Dxsk](https://explorer.solana.com/tx/34URVpdWmsNNbFE5d9b5CqZWgEfucjqjitgzGdZ1KvTd33YAgJpTGgSrqJjfMqrLe86X1YgKe2rZLgD3ipQ6Dxsk?cluster=devnet) |
| Tenant pays rent | Passed; 1 mock USDC entered rent vault and `current_deposits` | [2v42kA3Dpo8sqvfErUesXYX3E35LcTBj3TobS5jaKCyzKxy7SNYy9dGKJZcx2H9wg1tuHqeAHv5pjrc1B47zT2u1](https://explorer.solana.com/tx/2v42kA3Dpo8sqvfErUesXYX3E35LcTBj3TobS5jaKCyzKxy7SNYy9dGKJZcx2H9wg1tuHqeAHv5pjrc1B47zT2u1?cluster=devnet) |
| Finalize rent after real cutoff | Passed; RentPeriod created with 0.8 investor / 0.2 owner mock USDC allocation | [4W68FbtBmfQW6WwZQRcPmye5Wue8WyKmCshFC38ozZ5Xpp6xm9DoWjnjYj8pkBT2yYQWEg6iatnYqJQKXXUSiVWv](https://explorer.solana.com/tx/4W68FbtBmfQW6WwZQRcPmye5Wue8WyKmCshFC38ozZ5Xpp6xm9DoWjnjYj8pkBT2yYQWEg6iatnYqJQKXXUSiVWv?cluster=devnet) |
| Investor claims rent | Passed; 0.8 mock USDC received; claim PDA and accounting verified | [5aNYo2q1amSyF76fVAQXmgxcCtfSwJJLxpmLbTkBhcMZyX82qYHj7hvhnPqqxeH6rDdqNPQUfhCSHnxf8FDcaKMg](https://explorer.solana.com/tx/5aNYo2q1amSyF76fVAQXmgxcCtfSwJJLxpmLbTkBhcMZyX82qYHj7hvhnPqqxeH6rDdqNPQUfhCSHnxf8FDcaKMg?cluster=devnet) |
| Reject duplicate investor claim | Passed; confirmed `RentAlreadyClaimed`; state unchanged | [63WQFR6ZLsdBJL8MRozog4nK6iUkyV8Q851KHJcB63WTHd7tkt6jqgj339A9unzEpz21Rn1ot7CqnW81ymhqkNBW](https://explorer.solana.com/tx/63WQFR6ZLsdBJL8MRozog4nK6iUkyV8Q851KHJcB63WTHd7tkt6jqgj339A9unzEpz21Rn1ot7CqnW81ymhqkNBW?cluster=devnet) |
| Owner claims rent | Passed; 0.2 mock USDC received; rent vault and owner accounting verified | [3gLX3agSKSrtTuKVBrr7WeoKQakynnAmY3SU6LEQcajyrLD4RxfoZqcJ8Tz8mGXDEe1b825NvEMVDgFTx1F7iXfr](https://explorer.solana.com/tx/3gLX3agSKSrtTuKVBrr7WeoKQakynnAmY3SU6LEQcajyrLD4RxfoZqcJ8Tz8mGXDEe1b825NvEMVDgFTx1F7iXfr?cluster=devnet) |

## Compatibility and isolation evidence

- The saved compatibility IDL matches the restored Rust listing and investment argument order. Confirmed historical Devnet instructions have matching discriminators, account counts, and data lengths: listing 17 accounts / 106 bytes, investment 19 accounts / 50 bytes.
- The repository test IDL fixtures are byte-for-byte copies of the locally verified compatibility IDL and Transfer Hook IDL. The suite checks them against local `target/idl` copies when present.
- Demo Heights and Martinator Heights each decoded as a 229-byte `Property` account owned by the main program. Their account-data SHA-256 hashes were unchanged before and after this isolated test run.
- The main program remained executable at deployment slot `509839771`; the Transfer Hook remained at slot `509840173`. No deployment or contract edit occurred during testing.
- The initial exploratory run produced 13 passing tests and one failed *test assertion*: the unauthorized withdrawal correctly returned the contract's `Unauthorized` error, while the test expected Anchor's generic `ConstraintHasOne`. That assertion was corrected, and the full suite was rerun cleanly. The first run also created isolated Property PDA `2AtDQYBgTryEoKr4GfvxHQ6WM5JwMcDpRFSPJCNrmTBL`; it did not touch either existing demo Property account.

The test investor wallet exists only in process memory. Re-running the suite creates a new investor and property and consumes a small amount of Devnet SOL and mock USDC. The recorded results above are the **final clean run**; they do not claim that an earlier failed assertion passed.

These tests verify the deployed contracts directly. They are not a browser frontend test, and the current detached frontend's demo-only controls are outside this evidence.
