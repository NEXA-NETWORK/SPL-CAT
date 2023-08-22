import * as anchor from "@coral-xyz/anchor";
import { assert, expect } from "chai";
import { Program } from "@coral-xyz/anchor";
import { CatSol20 } from "../../target/types/cat_sol20";
import { TOKEN_METADATA_PROGRAM_ID } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { deriveAddress } from "@certusone/wormhole-sdk/lib/cjs/solana";
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createSetAuthorityInstruction,
  AuthorityType,
  createMintToInstruction,
} from "@solana/spl-token"
import { Keypair, LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import {
  getEmitterAddressEth,
  getEmitterAddressSolana,
  tryNativeToUint8Array,
  tryHexToNativeString,
  parseSequenceFromLogSolana,
  postVaaSolanaWithRetry,
  getSignedVAAHash,
  CHAINS,
  parseVaa,
  tryUint8ArrayToNative,
} from '@certusone/wormhole-sdk';
import { getWormholeCpiAccounts, getPostMessageCpiAccounts } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getProgramSequenceTracker, derivePostedVaaKey, SequenceTracker } from "@certusone/wormhole-sdk/lib/cjs/solana/wormhole";
import { getPostMessageAccounts, getEmitterKeys, deserializeSequenceTracker } from "./utils";
import axios from "axios";
import fs from "fs";
import { exec } from 'child_process';
import path from 'path';

function deployProgram() {
  const scriptPath = path.resolve(process.cwd(), 'migrations/deploy.sh');

  return new Promise((resolve, reject) => {
    exec(scriptPath, (error, stdout, stderr) => {
      if (error) {
        console.error(`An error occurred: ${error}`);
        reject(error);
        return;
      }
      console.log(`STDOUT: ${stdout}`);
      console.error(`STDERR: ${stderr}`);
      resolve(stdout);
    });
  });
}



describe("cat_sol20", () => {
  const provider = anchor.AnchorProvider.env();
  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  const program = anchor.workspace.CatSol20 as Program<CatSol20>;

  const SPL_CAT_PID = program.programId;

  const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";

  // The Owner of the token mint
  const NEXA_KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));

  // The new owner of the token mint
  const USER_KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id2.json').toString())));

  // The Token Mint we will use for testing
  const tokenMintPDA = PublicKey.findProgramAddressSync([Buffer.from("spl_cat_token"), USER_KEYPAIR.publicKey.toBuffer()], SPL_CAT_PID)[0];

  // The Token Metadata PDA
  const tokenMetadataPDA = PublicKey.findProgramAddressSync([Buffer.from("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), tokenMintPDA.toBuffer()], TOKEN_METADATA_PROGRAM_ID)[0];

  // The Bridge out VAA will be saved here and used for Bridge In
  let VAA: any = null;

  before(async () => {
    try {
      const tx = await provider.connection.requestAirdrop(USER_KEYPAIR.publicKey, 10 * LAMPORTS_PER_SOL);
      console.log("Fund AirDrop Transaction: ", tx);
    } catch (e: any) {
      console.log(e);
    }
  });


  describe("Initialization and Minting", () => {
    it("Can Initialize and Create a Mint", async () => {
      try {
        const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("config"),
          USER_KEYPAIR.publicKey.toBuffer(),
        ], SPL_CAT_PID);

        // Initial Sequence is 1
        const initial_sequence = Buffer.alloc(8);
        initial_sequence.writeBigUint64LE(BigInt(1));

        const wormhole = getPostMessageAccounts(
          CORE_BRIDGE_PID,
          SPL_CAT_PID,
          tokenMintPDA,
          1  // Initial Sequence is one
        );

        let max_supply = new anchor.BN("10000000000000000000");
        // let max_supply = new anchor.BN("0");


        const method = program.methods.initialize({
          decimals: 9,
          maxSupply: max_supply,
          name: "Cat Token",
          symbol: "CAT",
          uri: "",
        }).accounts({
          payer: NEXA_KEYPAIR.publicKey,
          owner: USER_KEYPAIR.publicKey,
          config: configAcc,
          tokenMint: tokenMintPDA,
          metadataAccount: tokenMetadataPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          metadataProgram: TOKEN_METADATA_PROGRAM_ID,
          wormholeProgram: CORE_BRIDGE_PID,
          wormholeBridge: wormhole.bridge,
          wormholeEmitter: wormhole.emitter,
          wormholeSequence: wormhole.sequence,
          wormholeFeeCollector: wormhole.feeCollector,
          wormholeMessage: wormhole.message,
          clock: wormhole.clock,
          rent: wormhole.rent,
          systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([NEXA_KEYPAIR, USER_KEYPAIR]);

        const tx = await method.transaction();
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = NEXA_KEYPAIR.publicKey;
        const message = tx.compileMessage();

        const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
        console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

        const simulate = await provider.connection.simulateTransaction(tx);
        console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

        const rpc = await method.rpc();
        console.log("Your transaction signature", rpc);

        // Check the config account
        const configAccount = await program.account.config.fetch(configAcc);

        assert.ok(configAccount.maxSupply.eq(max_supply));

      } catch (e: any) {
        console.log(e);
      }
    });

    it("Can Mint Tokens", async () => {
      try {

        const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("config"),
          USER_KEYPAIR.publicKey.toBuffer(),
        ], SPL_CAT_PID);

        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          USER_KEYPAIR.publicKey,
        );

        let amount = new anchor.BN("100000000000000000");
        const method = program.methods.mintTokens(amount).accounts({
          payer: NEXA_KEYPAIR.publicKey,
          owner: USER_KEYPAIR.publicKey,
          config: configAcc,
          tokenMint: tokenMintPDA,
          tokenUserAta: tokenUserATA,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([NEXA_KEYPAIR]);

        const tx = await method.transaction();
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = NEXA_KEYPAIR.publicKey;
        const message = tx.compileMessage();

        const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
        console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

        const simulate = await provider.connection.simulateTransaction(tx);
        console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

        const rpc = await method.rpc();
        console.log("Your transaction signature", rpc);
      } catch (e: any) {
        console.log(e);
      }
    });
  });


  // describe("Ownership Transfers", () => {
  //   it("Can Transfer Config Ownership", async () => {
  //     const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //       Buffer.from("config"),
  //       USER_KEYPAIR.publicKey.toBuffer(),
  //     ], SPL_CAT_PID);

  //     const method = program.methods.transferOwnership().accounts({
  //       owner: NEXA_KEYPAIR.publicKey,
  //       user: USER_KEYPAIR.publicKey,
  //       config: configAcc,
  //     }).signers([NEXA_KEYPAIR]);

  //     const tx = await method.transaction();
  //     tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
  //     tx.feePayer = NEXA_KEYPAIR.publicKey;
  //     const message = tx.compileMessage();

  //     const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
  //     console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

  //     const simulate = await provider.connection.simulateTransaction(tx);
  //     console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

  //     const rpc = await method.rpc();
  //     console.log("Your transaction signature", rpc);

  //     // You can assert that the transaction was successful.
  //     assert.ok(rpc, "Transaction failed");
  //   });

  //   it("Should Fail to Transfer Ownership to Existing Owner", async () => {
  //     try {
  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       const tx = await program.methods.transferOwnership().accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: USER_KEYPAIR.publicKey, // Using the same owner here
  //         config: configAcc,
  //       }).signers([USER_KEYPAIR]).rpc();

  //       // If no error was thrown by the previous code, this assertion will fail the test
  //       expect.fail("Transfer to existing owner should have failed, but it succeeded");
  //     } catch (e: any) {
  //       // If an error was thrown, we'll assert that it's the error we expected
  //       expect(e.message).to.include("AlreadyOwner");
  //     }
  //   });


  //   it("Can Mint Tokens With New Owner", async () => {
  //     try {

  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       const tokenUserATA = getAssociatedTokenAddressSync(
  //         tokenMintPDA,
  //         USER_KEYPAIR.publicKey,
  //       );

  //       let amount = new anchor.BN("100000000000000000");
  //       const method = await program.methods.mintTokens(amount).accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: USER_KEYPAIR.publicKey,
  //         config: configAcc,
  //         tokenMint: tokenMintPDA,
  //         tokenUserAta: tokenUserATA,
  //         tokenProgram: TOKEN_PROGRAM_ID,
  //         associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  //         systemProgram: anchor.web3.SystemProgram.programId,
  //       }).signers([USER_KEYPAIR]);

  //       const tx = await method.transaction();

  //       tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
  //       tx.feePayer = NEXA_KEYPAIR.publicKey;
  //       const message = tx.compileMessage();

  //       const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
  //       console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

  //       const simulate = await provider.connection.simulateTransaction(tx);
  //       console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

  //       const rpc = await method.rpc();
  //       console.log("Your transaction signature", rpc);

  //     } catch (e: any) {
  //       console.log(e);
  //     }
  //   })
  // });

  // describe("Registering Chains", () => {

  //   it("Can Register a chain", async () => {
  //     try {

  //       // Replace this with the Eth Contract
  //       const ethContractAddress = "0x970e8f18ebfEa0B08810f33a5A40438b9530FBCF";
  //       let targetEmitterAddress: string | number[] = getEmitterAddressEth(ethContractAddress);
  //       targetEmitterAddress = Array.from(Buffer.from(targetEmitterAddress, "hex"))

  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       // Registering Ethereum 
  //       const foreignChainId = Buffer.alloc(2);
  //       foreignChainId.writeUInt16LE(CHAINS.ethereum);

  //       const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("foreign_emitter"),
  //         configAcc.toBuffer(),
  //         foreignChainId,
  //       ], SPL_CAT_PID)

  //       const method = program.methods.registerEmitter({
  //         chain: CHAINS.ethereum,
  //         address: targetEmitterAddress,
  //       }).accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: USER_KEYPAIR.publicKey,
  //         config: configAcc,
  //         foreignEmitter: emitterAcc,
  //         systemProgram: anchor.web3.SystemProgram.programId
  //       })
  //         .signers([USER_KEYPAIR])

  //       const tx = await method.transaction();

  //       tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
  //       tx.feePayer = NEXA_KEYPAIR.publicKey;
  //       const message = tx.compileMessage();

  //       const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
  //       console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

  //       const simulate = await provider.connection.simulateTransaction(tx);
  //       console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

  //       const rpc = await method.rpc();
  //       console.log("Your transaction signature", rpc);

  //       assert.ok(rpc, "Transaction failed to register a chain");

  //     } catch (e: any) {
  //       console.log(e);
  //       assert.fail(`Unexpected error occurred: ${e.message}`);
  //     }
  //   })

  //   it("Should Fail to Register a chain with an invalid chain", async () => {
  //     try {

  //       let targetEmitterAddress: string | number[] = getEmitterAddressSolana(SPL_CAT_PID);
  //       targetEmitterAddress = Array.from(Buffer.from(targetEmitterAddress, "hex"))

  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       // Registering Ethereum 
  //       const foreignChainId = Buffer.alloc(2);
  //       foreignChainId.writeUInt16LE(CHAINS.solana);

  //       const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("foreign_emitter"),
  //         configAcc.toBuffer(),
  //         foreignChainId,
  //       ], SPL_CAT_PID)

  //       const tx = await program.methods.registerEmitter({
  //         chain: CHAINS.solana,
  //         address: targetEmitterAddress,
  //       }).accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: USER_KEYPAIR.publicKey,
  //         config: configAcc,
  //         foreignEmitter: emitterAcc,
  //         systemProgram: anchor.web3.SystemProgram.programId
  //       })
  //         .signers([USER_KEYPAIR])
  //         .rpc();

  //       console.log("Your transaction signature", tx);
  //       expect.fail("Chain registration should have failed, but it succeeded");
  //     } catch (e: any) {
  //       expect(e.message).to.include("Invalid Chain ID or Zero Address");
  //     }
  //   })

  // });

  // describe("Bridging", () => {
  //   it("Can Bridge Out", async () => {
  //     try {
  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       // Make Sure this acc is initialized and has tokens
  //       const tokenUserATA = getAssociatedTokenAddressSync(
  //         tokenMintPDA,
  //         USER_KEYPAIR.publicKey,
  //       );

  //       const foreignChainId = Buffer.alloc(2);
  //       foreignChainId.writeUInt16LE(CHAINS.ethereum);

  //       const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("foreign_emitter"),
  //         configAcc.toBuffer(),
  //         foreignChainId,
  //       ], SPL_CAT_PID)

  //       const { emitter, sequence } = getEmitterKeys(SPL_CAT_PID, CORE_BRIDGE_PID, tokenMintPDA);
  //       const sequenceAccount = await provider.connection.getAccountInfo(sequence);
  //       const sequenceNumber = deserializeSequenceTracker(sequenceAccount.data);

  //       const wormholeAccounts = getPostMessageAccounts(
  //         CORE_BRIDGE_PID,
  //         SPL_CAT_PID,
  //         tokenMintPDA,
  //         Number(sequenceNumber) + 1,
  //       );

  //       // User's Ethereum address
  //       let userEthAddress = "0x90F8bf6A479f320ead074411a4B0e7944Ea8c9C1";
  //       let recipient = Array.from(tryNativeToUint8Array(userEthAddress, "ethereum"));

  //       // Parameters
  //       let amount = new anchor.BN("10000000000000000");
  //       let recipientChain = 2;
  //       const method = program.methods.bridgeOut({
  //         amount,
  //         recipientChain,
  //         recipient,
  //       }).accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: USER_KEYPAIR.publicKey,
  //         tokenUserAta: tokenUserATA,
  //         tokenMint: tokenMintPDA,
  //         tokenProgram: TOKEN_PROGRAM_ID,
  //         associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  //         wormholeProgram: CORE_BRIDGE_PID,
  //         foreignEmitter: emitterAcc,
  //         config: configAcc,
  //         wormholeBridge: wormholeAccounts.bridge,
  //         wormholeEmitter: wormholeAccounts.emitter,
  //         wormholeFeeCollector: wormholeAccounts.feeCollector,
  //         wormholeMessage: wormholeAccounts.message,
  //         wormholeSequence: wormholeAccounts.sequence,
  //         clock: wormholeAccounts.clock,
  //         rent: wormholeAccounts.rent,
  //         systemProgram: wormholeAccounts.systemProgram
  //       }).signers([USER_KEYPAIR])

  //       // const tx = await method.transaction();

  //       // tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
  //       // tx.feePayer = NEXA_KEYPAIR.publicKey;
  //       // const message = tx.compileMessage();

  //       // const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
  //       // console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

  //       // const simulate = await provider.connection.simulateTransaction(tx);
  //       // console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

  //       const rpc = await method.rpc();
  //       console.log("Your transaction signature", rpc);

  //       await new Promise((r) => setTimeout(r, 3000)); // Wait for tx to be confirmed

  //       const confirmedTx = await provider.connection.getTransaction(rpc, { commitment: "confirmed", maxSupportedTransactionVersion: 2 });

  //       const seq = parseSequenceFromLogSolana(confirmedTx)
  //       const emitterAddr = wormholeAccounts.emitter.toBuffer().toString("hex"); 

  //       console.log("Sequence: ", seq);
  //       console.log("Emitter Address: ", emitterAddr);

  //       await new Promise((r) => setTimeout(r, 3000)); // Wait for guardian to pick up message
  //       const restAddress = "http://localhost:7071"

  //       console.log(
  //         "Searching for: ",
  //         `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
  //       );

  //       const vaaBytes = await axios.get(
  //         `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
  //       );

  //       VAA = vaaBytes.data.vaaBytes;
  //       console.log("VAA Bytes: ", vaaBytes.data);

  //     } catch (e: any) {
  //       console.log(e);
  //     }
  //   });

  //   it("Bridge In", async () => {
  //     try {
  //       await postVaaSolanaWithRetry(
  //         provider.connection,
  //         async (tx) => {
  //           tx.partialSign(USER_KEYPAIR);
  //           return tx;
  //         },
  //         CORE_BRIDGE_PID,
  //         USER_KEYPAIR.publicKey.toString(),
  //         Buffer.from(VAA, "base64"),
  //         10
  //       );

  //       const parsedVAA = parseVaa(Buffer.from(VAA, 'base64'));
  //       const payload = getParsedPayload(parsedVAA.payload);

  //       const postedVAAKey = derivePostedVaaKey(CORE_BRIDGE_PID, parsedVAA.hash);
  //       const recievedKey = PublicKey.findProgramAddressSync(
  //         [
  //           Buffer.from("received"),
  //           (() => {
  //             const buf = Buffer.alloc(10);
  //             buf.writeUInt16LE(parsedVAA.emitterChain, 0);
  //             buf.writeBigInt64LE(parsedVAA.sequence, 2);
  //             return buf;
  //           })(),
  //         ], SPL_CAT_PID)[0];


  //       const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("config"),
  //         USER_KEYPAIR.publicKey.toBuffer(),
  //       ], SPL_CAT_PID);

  //       const tokenUserATA = getAssociatedTokenAddressSync(
  //         tokenMintPDA,
  //         payload.toAddress,
  //       );

  //       const foreignChainId = Buffer.alloc(2);
  //       foreignChainId.writeUInt16LE(CHAINS.ethereum);

  //       const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
  //         Buffer.from("foreign_emitter"),
  //         configAcc.toBuffer(),
  //         foreignChainId,
  //       ], SPL_CAT_PID)

  //       const method = program.methods.bridgeIn(Array.from(parsedVAA.hash)).accounts({
  //         owner: USER_KEYPAIR.publicKey,
  //         user: payload.toAddress,
  //         tokenUserAta: tokenUserATA,
  //         tokenMint: tokenMintPDA,
  //         tokenProgram: TOKEN_PROGRAM_ID,
  //         associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  //         wormholeProgram: CORE_BRIDGE_PID,
  //         foreignEmitter: emitterAcc,
  //         posted: postedVAAKey,
  //         received: recievedKey,
  //         config: configAcc,
  //         systemProgram: anchor.web3.SystemProgram.programId,
  //       }).signers([USER_KEYPAIR]);

  //       const tx = await method.transaction();
  //       tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
  //       tx.feePayer = NEXA_KEYPAIR.publicKey;
  //       const message = tx.compileMessage();

  //       const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
  //       console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

  //       const simulate = await provider.connection.simulateTransaction(tx);
  //       console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

  //       const rpc = await method.rpc();
  //       console.log("Your transaction signature", rpc);

  //     } catch (e: any) {
  //       console.log(e);
  //     }
  //   });

  // });

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