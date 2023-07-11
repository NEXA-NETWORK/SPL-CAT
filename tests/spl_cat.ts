import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SplCat } from "../target/types/spl_cat";
import { deriveAddress } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token"
import { PublicKey } from "@solana/web3.js";
import {
  getEmitterAddressEth,
  getEmitterAddressSolana,
  parseSequenceFromLogSolana,
  postVaaSolanaWithRetry,
  getSignedVAAHash,
  CHAINS,
  ChainId,
  isBytes,

  parseVaa,
} from '@certusone/wormhole-sdk';
import { getWormholeCpiAccounts, getPostMessageCpiAccounts } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getProgramSequenceTracker, derivePostedVaaKey } from "@certusone/wormhole-sdk/lib/cjs/solana/wormhole";
import axios from "axios";
import fs from "fs";


describe("spl_cat", () => {
  const provider = anchor.AnchorProvider.env();
  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  const program = anchor.workspace.SplCat as Program<SplCat>;
  const SPL_CAT_PID = program.programId;
  const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";

  // For Testing we're going to use the Solana
  const targetChainId = Buffer.alloc(2);
  targetChainId.writeUInt16LE(CHAINS.solana);
  const targetEmitter = PublicKey.findProgramAddressSync([Buffer.from("emitter")], SPL_CAT_PID)[0];

  // The Owner of the token mint
  const KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));

  // The Token Mint we will use for testing
  const tokenMintPDA = PublicKey.findProgramAddressSync([Buffer.from("spl_cat_token")], program.programId)[0];

  let VAA: any = null;

  it("Can Initialize and Create a Mint", async () => {
    try {
      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PID);

      const tokenAccountPDA = getAssociatedTokenAddressSync(
        tokenMintPDA,
        KEYPAIR.publicKey,
      );
      const initial_sequence = Buffer.alloc(8);
      initial_sequence.writeBigUint64LE(BigInt(1));

      const wormhole = getWormholeCpiAccounts(
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey,
        SPL_CAT_PID,
        deriveAddress([Buffer.from("sent"), initial_sequence], SPL_CAT_PID)
      );

      const tx = await program.methods.initialize(6, new anchor.BN(100000)).accounts({
        owner: KEYPAIR.publicKey,
        config: configAcc,
        tokenMint: tokenMintPDA,
        tokenAccount: tokenAccountPDA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        wormholeProgram: CORE_BRIDGE_PID,
        wormholeBridge: wormhole.bridge,
        wormholeEmitter: wormhole.emitter,
        wormholeSequence: wormhole.sequence,
        wormholeFeeCollector: wormhole.feeCollector,
        wormholeMessage: wormhole.message,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([KEYPAIR]).rpc();
      console.log("Your transaction signature", tx);
    } catch (e: any) {
      console.log(e);
    }
  });

  it("Can Register a chain", async () => {
    try {

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        targetChainId,
      ], SPL_CAT_PID)

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PID);

      // Decode the base58 string to a Buffer
      const targetEmitterBuffer = Array.from(targetEmitter.toBuffer());

      const tx = await program.methods.registerEmitter(CHAINS.solana, targetEmitterBuffer).accounts({
        owner: KEYPAIR.publicKey,
        config: configAcc,
        foreignEmitter: emitterAcc,
        systemProgram: anchor.web3.SystemProgram.programId
      })
        .signers([KEYPAIR])
        .rpc();

      console.log("Your transaction signature", tx);
    } catch (e: any) {

      console.log(e);
    }
  })


  it("Bridge Out", async () => {

    try {

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PID);

      const tokenAccountPDA = getAssociatedTokenAddressSync(
        tokenMintPDA,
        KEYPAIR.publicKey,
      );

      // get sequence
      const message = await getProgramSequenceTracker(provider.connection, SPL_CAT_PID, CORE_BRIDGE_PID)
        .then((tracker) =>
          deriveAddress(
            [
              Buffer.from("sent"),
              (() => {
                const buf = Buffer.alloc(8);
                buf.writeBigUInt64LE(tracker.sequence + BigInt(1));
                return buf;
              })(),
            ],
            SPL_CAT_PID
          )
        );

      const wormholeAccounts = getPostMessageCpiAccounts(
        SPL_CAT_PID,
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey,
        message
      );

      // User's Ethereum address
      // let userEthAddress = "0xf4ec5E2FB5085ae171Ca61818760812CA3Fb2dCa";
      // let recipient = Array.from(Web3.utils.hexToBytes(Web3.utils.padLeft(userEthAddress, 64))); // pad the address to 32 bytes

      // Parameters
      let amount = new anchor.BN(50000); // Deducting Half
      let recipientChain = 1; // ETH mainnet
      let recipient = Array.from(KEYPAIR.publicKey.toBuffer()); // Using the solarium wallet address for testing

      const tx = await program.methods.bridgeOut(amount, recipientChain, recipient).accounts({
        owner: KEYPAIR.publicKey,
        // Token Stuff
        tokenAccount: tokenAccountPDA,
        tokenMint: tokenMintPDA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        // Wormhole Stuff
        wormholeProgram: CORE_BRIDGE_PID,
        config: configAcc,
        ...wormholeAccounts,
      }).signers([KEYPAIR]).rpc();

      console.log("Your transaction signature", tx);
      await new Promise((r) => setTimeout(r, 3000)); // Wait for tx to be confirmed

      const confirmedTx = await provider.connection.getTransaction(tx, { commitment: "confirmed", maxSupportedTransactionVersion: 2 });

      const seq = parseSequenceFromLogSolana(confirmedTx)
      const emitterAddr = getEmitterAddressSolana(program.programId.toString()); //same as whDerivedEmitter

      // console.log("Sequence: ", seq);
      // console.log("Emitter Address: ", emitterAddr);

      await new Promise((r) => setTimeout(r, 3000)); // Wait for guardian to pick up message
      const restAddress = "http://localhost:7071"

      // console.log(
      //   "Searching for: ",
      //   `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
      // );

      const vaaBytes = await axios.get(
        `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
      );
      VAA = vaaBytes.data.vaaBytes;
      console.log("VAA Bytes: ", vaaBytes.data);

    } catch (e: any) {
      console.log(e);
    }
  });

  it("Bridge In", async () => {

    try {
      // VAA = 'AQAAAAABAP8WBltysjwnwFAwLiMlxndgtMArFWzYiR3peeyyX1SuF287PLW1RZfJjnuAy/s1lvyTzGyWupsB83RSLUqA34IBZK0ZaQAAAAAAARH0Tq4CvzN0nH8fNSFjnbyHavb+3wZYEz2FX0xIWVR0AAAAAAAAAAEBUMMAAAAAAABjgLKiAJRtxGQfA+X5v38PQ7S0pF9Q0TBIewHeBzSOYIinVTQ3ptO2PVOEuWKhQHEZVe0Z4B5MJhYx2dvBguHYAQABAA=='

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PID);

      const tokenAccountPDA = getAssociatedTokenAddressSync(
        tokenMintPDA,
        KEYPAIR.publicKey,
      );

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        targetChainId,
      ], SPL_CAT_PID)

      await postVaaSolanaWithRetry(
        provider.connection,
        async (tx) => {
          tx.partialSign(KEYPAIR);
          return tx;
        },
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey.toString(),
        Buffer.from(VAA, "base64"),
        10
      );

      const parsedVAA = parseVaa(Buffer.from(VAA, 'base64'));
      const postedVAAKey = derivePostedVaaKey(CORE_BRIDGE_PID, parsedVAA.hash);
      const recievedKey = PublicKey.findProgramAddressSync(
        [
          Buffer.from("received"),
          (() => {
            const buf = Buffer.alloc(10);
            buf.writeUInt16LE(parsedVAA.emitterChain, 0);
            buf.writeBigInt64LE(parsedVAA.sequence, 2);
            return buf;
          })(),
        ], SPL_CAT_PID)[0];


      const tx = await program.methods.bridgeIn(Array.from(parsedVAA.hash)).accounts({
        owner: KEYPAIR.publicKey,
        tokenAccount: tokenAccountPDA,
        tokenMint: tokenMintPDA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        wormholeProgram: CORE_BRIDGE_PID,
        foreignEmitter: emitterAcc,
        posted: postedVAAKey,
        received: recievedKey,
        config: configAcc,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([KEYPAIR]).rpc();

      console.log("Your transaction signature", tx);
    } catch (e: any) {
      console.log(e);
    }
  });
});
