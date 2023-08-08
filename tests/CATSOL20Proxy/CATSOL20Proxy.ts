import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CatSol20Proxy } from "../../target/types/cat_sol20_proxy";
import { TestToken } from "../../target/types/test_token";
import { TOKEN_METADATA_PROGRAM_ID } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { deriveAddress } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, approve, createApproveInstruction } from "@solana/spl-token"
import { PublicKey } from "@solana/web3.js";
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import {
  getEmitterAddressEth,
  getEmitterAddressSolana,
  parseSequenceFromLogSolana,
  postVaaSolanaWithRetry,
  tryNativeToUint8Array,
  getSignedVAAHash,
  CHAINS,
  parseVaa,
} from '@certusone/wormhole-sdk';
import { getWormholeCpiAccounts, getPostMessageCpiAccounts } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getProgramSequenceTracker, derivePostedVaaKey } from "@certusone/wormhole-sdk/lib/cjs/solana/wormhole";
import axios from "axios";
import fs from "fs";


describe("cat_sol20_proxy", () => {
  const provider = anchor.AnchorProvider.env();
  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  /// --------------------------------------------- TEST TOKEN --------------------------------------------- ///
  /// Program
  const testTokenProgram = anchor.workspace.TestToken as Program<TestToken>;
  const TEST_SPL_PID = testTokenProgram.programId;
  /// Test Keypair
  const TEST_KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/test.json').toString())));
  /// Token Creation
  const TEST_SPL_SEED = Buffer.from("test_token");
  const testTokenMintPDA = PublicKey.findProgramAddressSync([TEST_SPL_SEED], TEST_SPL_PID)[0];
  const testTokenUserATA = getAssociatedTokenAddressSync(
    testTokenMintPDA,
    TEST_KEYPAIR.publicKey,
  );
  // Token Decimals
  const decimals = 9;
  const ten = new anchor.BN(10);
  const oneToken = new anchor.BN(1).mul(ten.pow(new anchor.BN(decimals)));

  /// ------------------------------------------- PROXY CONTRACT ------------------------------------------- ///
  /// Program
  const program = anchor.workspace.CatSol20Proxy as Program<CatSol20Proxy>;
  const SPL_CAT_PROXY_PID = program.programId;
  /// Owner Keypair
  const KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));
  /// Seed to create and derive the PDA for Program Owned ATA that will lock the tokens
  const LOCK_PDA_SEED = Buffer.from("cat_sol_proxy");

  /// ---------------------------------------------- NEW OWNER --------------------------------------------- ///
  const NEW_OWNER_KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id2.json').toString())));

  /// ----------------------------------------------- WORMHOLE --------------------------------------------- ///
  const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";
  // The Bridge out VAA will be saved here and used for Bridge In
  let VAA: any = null;

  it("Fund New owner with some SOL", async () => {
    try {
      const newOwnertx = await provider.connection.requestAirdrop(NEW_OWNER_KEYPAIR.publicKey, 100 * LAMPORTS_PER_SOL);
      console.log("Your newOwnertx transaction signature", newOwnertx);

      const testKeypair = await provider.connection.requestAirdrop(TEST_KEYPAIR.publicKey, 100 * LAMPORTS_PER_SOL);
      console.log("Your testKeypair transaction signature", testKeypair);

      await new Promise((r) => setTimeout(r, 3000)); // Wait for tx to be finalized
    } catch (e: any) {
      console.log(e);
    }
  });

  it("TEST TOKEN: Initialize & Mint", async () => {
    try {

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], TEST_SPL_PID);

      const max_supply = new anchor.BN(100000).mul(oneToken);
      const amount = new anchor.BN(100000).mul(oneToken);

      const tx = await testTokenProgram.methods.initialize(decimals, max_supply, amount).accounts({
        owner: TEST_KEYPAIR.publicKey,
        ataAuthority: TEST_KEYPAIR.publicKey,
        config: configAcc,
        tokenMint: testTokenMintPDA,
        tokenUserAta: testTokenUserATA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([TEST_KEYPAIR]).rpc();
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
      // Registering Ethereum 
      const foreignChainId = Buffer.alloc(2);
      foreignChainId.writeUInt16LE(CHAINS.ethereum);

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        foreignChainId,
      ], SPL_CAT_PROXY_PID);

      // Replace this with the Eth Contract
      const ethContractAddress = "0xDb56f2e9369E0D7bD191099125a3f6C370F8ed15";
      let targetEmitterAddress: string | number[] = getEmitterAddressEth(ethContractAddress);
      console.log("Target Emitter Address: ", targetEmitterAddress);
      targetEmitterAddress = Array.from(Buffer.from(targetEmitterAddress, "hex"))

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PROXY_PID);

      const tx = await program.methods.registerEmitter({
        chain: CHAINS.ethereum,
        address: targetEmitterAddress,
      }).accounts({
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

      const foreignChainId = Buffer.alloc(2);
      foreignChainId.writeUInt16LE(CHAINS.ethereum);

      const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        foreignChainId,
      ], SPL_CAT_PROXY_PID);

      const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PROXY_PID);

      // PDA for Locking the Tokens
      const tokenATAPDA = PublicKey.findProgramAddressSync([LOCK_PDA_SEED, testTokenUserATA.toBuffer()], SPL_CAT_PROXY_PID)[0];
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
      let userEthAddress = "0x90F8bf6A479f320ead074411a4B0e7944Ea8c9C1";
      let recipient = Array.from(tryNativeToUint8Array(userEthAddress, "ethereum"));

      // Parameters

      let amount = new anchor.BN(10000).mul(oneToken);
      let recipientChain = 2;

      // Approve
      const transaction = new anchor.web3.Transaction();
      transaction.add(
        createApproveInstruction(
          testTokenUserATA,
          tokenATAPDA,
          TEST_KEYPAIR.publicKey,
          BigInt(amount.toString())
        )
      );

      const approveTx = await anchor.web3.sendAndConfirmTransaction(provider.connection, transaction, [TEST_KEYPAIR])
      console.log("Your Approve transaction signature", approveTx);

      // Bridge Out
      const tx = await program.methods.bridgeOut({
        amount,
        recipientChain,
        recipient,
      }).accounts({
        owner: KEYPAIR.publicKey,
        // Token Stuff
        tokenMint: testTokenMintPDA,
        tokenUserAta: testTokenUserATA,
        tokenAtaPda: tokenATAPDA,
        tokenMintAta: tokenMintATA,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        // Wormhole Stuff
        wormholeProgram: CORE_BRIDGE_PID,
        foreignEmitter: emitterAcc,
        config: configAcc,
        ...wormholeAccounts,
      }).signers([KEYPAIR]).rpc();

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
      console.log("Payload: ", payload);

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
      const tokenATAPDA = PublicKey.findProgramAddressSync([LOCK_PDA_SEED, testTokenUserATA.toBuffer()], SPL_CAT_PROXY_PID)[0];
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
    tokenChain: tokenChain.readUInt16BE(),
    toAddress: new PublicKey(toAddress),
    toChain: toChain.readUInt16BE(),
    tokenDecimals: tokenDecimals.readUInt8()
  }
}