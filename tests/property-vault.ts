import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { PropertyVault } from "../target/types/property_vault";

describe("property-vault", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.propertyVault as Program<PropertyVault>;

  // it("Initializes and increments a counter", async () => {
  //   const [counter] = anchor.web3.PublicKey.findProgramAddressSync(
  //     [Buffer.from("counter")],
  //     program.programId
  //   );

  //   const initializeTx = await program.methods
  //     .initialize()
  //     .accountsPartial({ counter })
  //     .rpc();
  //   console.log("Initialize transaction signature", initializeTx);

  //   const incrementTx = await program.methods
  //     .increment()
  //     .accountsPartial({ counter })
  //     .rpc();
  //   console.log("Increment transaction signature", incrementTx);
  // });
});
