import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { spl_cat } from "../target/types/spl_cat";

describe("spl_cat", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.spl_cat as Program<spl_cat>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
