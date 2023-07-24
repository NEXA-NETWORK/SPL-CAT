import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CatSol20Proxy } from "../target/types/cat_sol20_proxy";
import { TestToken } from "../target/types/test_token";
import { TOKEN_METADATA_PROGRAM_ID } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { deriveAddress } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token"
import { PublicKey } from "@solana/web3.js";
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import {
  getEmitterAddressEth,
  getEmitterAddressSolana,
  parseSequenceFromLogSolana,
  postVaaSolanaWithRetry,
  getSignedVAAHash,
  CHAINS,
  parseVaa,
} from '@certusone/wormhole-sdk';
import { getWormholeCpiAccounts, getPostMessageCpiAccounts } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getProgramSequenceTracker, derivePostedVaaKey } from "@certusone/wormhole-sdk/lib/cjs/solana/wormhole";
import base58 from "bs58";
import axios from "axios";
import fs from "fs";


describe("Proxy CAT", () => {
  const provider = anchor.AnchorProvider.env();
  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  // Test Token 
  const testTokenProgram = anchor.workspace.TestToken as Program<TestToken>;
  const TEST_SPL_PID = testTokenProgram.programId;
  const TEST_SPL_SEED = Buffer.from("test_token");
  const testTokenMintPDA = PublicKey.findProgramAddressSync([TEST_SPL_SEED], TEST_SPL_PID)[0];
  const TEST_TOKEN_KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id2.json').toString())));
  const testTokenUserATA = getAssociatedTokenAddressSync(
    testTokenMintPDA,
    TEST_TOKEN_KEYPAIR.publicKey,
  );

  /// PROXY
  const program = anchor.workspace.CatSol20Proxy as Program<CatSol20Proxy>;
  const SPL_CAT_PROXY_PID = program.programId;
  const SPL_TOKEN_SEED = Buffer.from("cat_spl_token");
  const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";


  // The Owner of the token mint
  const KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));

  // The Bridge out VAA will be saved here and used for Bridge In
  let VAA: any = null;


  it("TEST TOKEN: Initialize & Mint", async () => {
    try {
      await provider.connection.confirmTransaction(
        await provider.connection.requestAirdrop(
          TEST_TOKEN_KEYPAIR.publicKey,
          1000 * LAMPORTS_PER_SOL // 2 SOL top-up
        )
      );
      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], TEST_SPL_PID);


      let max_supply = new anchor.BN("10000000000000000000");
      let amount = new anchor.BN("100000000000000000");


      const tx = await testTokenProgram.methods.initialize(9, max_supply, amount).accounts({
        owner: TEST_TOKEN_KEYPAIR.publicKey,
        ataAuthority: TEST_TOKEN_KEYPAIR.publicKey,
        config: configAcc,
        tokenMint: testTokenMintPDA,
        tokenUserAta: testTokenUserATA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([TEST_TOKEN_KEYPAIR]).rpc();
      console.log("Your transaction signature", tx);

    } catch (e: any) {
      console.log(e);
    }
  });

  it("Can Initialize and Create a Mint", async () => {
    try {
      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PROXY_PID);

      // Initial Sequence is 1
      const initial_sequence = Buffer.alloc(8);
      initial_sequence.writeBigUint64LE(BigInt(1));

      const wormhole = getWormholeCpiAccounts(
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey,
        SPL_CAT_PROXY_PID,
        deriveAddress([Buffer.from("sent"), initial_sequence], SPL_CAT_PROXY_PID)
      );

      const tx = await program.methods.initialize().accounts({
        owner: KEYPAIR.publicKey,
        // otherProgram: TEST_SPL_PID,
        config: configAcc,
        tokenMint: testTokenMintPDA,
        tokenProgram: TOKEN_PROGRAM_ID,
        wormholeProgram: CORE_BRIDGE_PID,
        wormholeBridge: wormhole.bridge,
        wormholeEmitter: wormhole.emitter,
        wormholeSequence: wormhole.sequence,
        wormholeFeeCollector: wormhole.feeCollector,
        wormholeMessage: wormhole.message,
        clock: wormhole.clock,
        rent: wormhole.rent,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([KEYPAIR]).rpc();
      console.log("Your transaction signature", tx);
    } catch (e: any) {
      console.log(e);
    }
  });

  it("Can Register a chain", async () => {
    try {
      // Registering Solana
      const foreignChainId = Buffer.alloc(2);
      foreignChainId.writeUInt16LE(CHAINS.solana);

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        foreignChainId,
      ], SPL_CAT_PROXY_PID)

      let targetEmitterAddress: string | number[] = Array.from(Buffer.from(getEmitterAddressSolana(SPL_CAT_PROXY_PID)));

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PROXY_PID);

      const tx = await program.methods.registerEmitter(CHAINS.solana, targetEmitterAddress).accounts({
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
      ], SPL_CAT_PROXY_PID);

      // PDA for Locking the Tokens
      const tokenATAPDA = PublicKey.findProgramAddressSync([SPL_TOKEN_SEED, testTokenUserATA.toBuffer()], SPL_CAT_PROXY_PID)[0];
      // ATA for Locking the Tokens
      const tokenMintATA = getAssociatedTokenAddressSync(
        testTokenMintPDA,
        tokenATAPDA,
        true
      );

      // get sequence
      const SequenceTracker = await getProgramSequenceTracker(provider.connection, SPL_CAT_PROXY_PID, CORE_BRIDGE_PID)
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
            SPL_CAT_PROXY_PID
          )
        );

      const wormholeAccounts = getPostMessageCpiAccounts(
        SPL_CAT_PROXY_PID,
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey,
        SequenceTracker
      );

      // User's Ethereum address
      // let userEthAddress = "0x90F8bf6A479f320ead074411a4B0e7944Ea8c9C1";
      // let recipient = Array.from(Buffer.from(userEthAddress.slice(2), "hex"))
      // // Pad to 32 bytes
      // while (recipient.length < 32) {
      //   recipient.unshift(0);
      // }

      // Parameters
      let amount = new anchor.BN("10000000000000000");
      let recipientChain = 1;
      let recipient = Array.from(TEST_TOKEN_KEYPAIR.publicKey.toBuffer())

      const tx = await program.methods.bridgeOut(amount, recipientChain, recipient).accounts({
        owner: KEYPAIR.publicKey,
        ataAuthority: TEST_TOKEN_KEYPAIR.publicKey,
        // Token Stuff
        tokenUserAta: testTokenUserATA,
        tokenAtaPda: tokenATAPDA,
        tokenMintAta: tokenMintATA,
        tokenMint: testTokenMintPDA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        // Wormhole Stuff
        wormholeProgram: CORE_BRIDGE_PID,
        config: configAcc,
        ...wormholeAccounts,
      }).signers([KEYPAIR, TEST_TOKEN_KEYPAIR]).rpc({ skipPreflight: true });

      console.log("Your transaction signature", tx);
      await new Promise((r) => setTimeout(r, 3000)); // Wait for tx to be confirmed

      const confirmedTx = await provider.connection.getTransaction(tx, { commitment: "confirmed", maxSupportedTransactionVersion: 2 });

      const seq = parseSequenceFromLogSolana(confirmedTx)
      const emitterAddr = getEmitterAddressSolana(SPL_CAT_PROXY_PID.toString()); //same as whDerivedEmitter

      console.log("Sequence: ", seq);
      console.log("Emitter Address: ", emitterAddr);

      await new Promise((r) => setTimeout(r, 3000)); // Wait for guardian to pick up message
      const restAddress = "http://localhost:7071"

      console.log(
        "Searching for: ",
        `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
      );

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
      const payload = getParsedPayload(parsedVAA.payload);

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
        ], SPL_CAT_PROXY_PID)[0];


      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PROXY_PID);

      const tokenUserATA = getAssociatedTokenAddressSync(
        testTokenMintPDA,
        payload.toAddress,
      );

      // PDA for Locking the Tokens
      const tokenATAPDA = PublicKey.findProgramAddressSync([SPL_TOKEN_SEED, testTokenUserATA.toBuffer()], SPL_CAT_PROXY_PID)[0];
      // ATA that holds the locked Tokens
      const tokenMintATA = getAssociatedTokenAddressSync(
        testTokenMintPDA,
        tokenATAPDA,
        true
      );

      const foreignChainId = Buffer.alloc(2);
      foreignChainId.writeUInt16LE(payload.tokenChain);

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        foreignChainId,
      ], SPL_CAT_PROXY_PID)

      const tx = await program.methods.bridgeIn(Array.from(parsedVAA.hash)).accounts({
        owner: KEYPAIR.publicKey,
        ataAuthority: payload.toAddress,
        tokenUserAta: tokenUserATA,
        tokenAtaPda: tokenATAPDA,
        tokenMintAta: tokenMintATA,
        tokenMint: testTokenMintPDA,
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


function getParsedPayload(vaa: Buffer) {
  let amount = vaa.subarray(0, 32);
  let tokenAddress = vaa.subarray(32, 64);
  let tokenChain = vaa.subarray(64, 66);
  let toAddress = vaa.subarray(66, 98);
  let toChain = vaa.subarray(98, 100);
  let tokenDecimals = vaa.subarray(100, 101);

  return {
    amount: BigInt(`0x${amount.toString('hex')}`),
    tokenAddress: tokenAddress.toString('hex'),
    tokenChain: tokenChain.readUInt16LE(),
    toAddress: new PublicKey(toAddress),
    toChain: toChain.readUInt16LE(),
    tokenDecimals: tokenDecimals.readUInt8()
  }
}