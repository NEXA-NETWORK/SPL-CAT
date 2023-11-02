import * as anchor from "@coral-xyz/anchor";
import { assert, expect } from "chai";
import { Program } from "@coral-xyz/anchor";
import { CatSol20 } from "../../target/types/cat_sol20";
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
import {
  getWormholeCpiAccounts,
  getPostMessageCpiAccounts, 
  TOKEN_METADATA_PROGRAM_ID, 
  deriveAddress
} from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getProgramSequenceTracker, derivePostedVaaKey } from "@certusone/wormhole-sdk/lib/cjs/solana/wormhole";
import axios from "axios";
import fs from "fs";
import { exec } from 'child_process';
import path from 'path';

function deployProgram(programName: string, cluster: string, wallet: string) {
  const scriptPath = path.resolve(process.cwd(), 'migrations/deploy.sh');
  const cmd = `${scriptPath} ${programName} ${cluster} ${wallet}`;

  return new Promise((resolve, reject) => {
    exec(cmd, (error, stdout, stderr) => {
      if (error) {
        console.error(`An error occurred: ${error}`);
        reject(error);
        return;
      }
      console.log(`STDOUT: ${stdout}`);
      if (stderr) {
        console.error(`STDERR: ${stderr}`);
      }
      resolve(stdout);
    });
  });
}


describe("cat_sol20", () => {
  const provider = anchor.AnchorProvider.env();
  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  let program: Program<CatSol20>;

  let SPL_CAT_PID: PublicKey;


  const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";

  // The Owner of the token mint
  const KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));

  // The new owner of the token mint
  const newOwner = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id2.json').toString())));

  // The Token Mint we will use for testing
  let tokenMintPDA;

  // The Token Metadata PDA
  let tokenMetadataPDA;

  // The Bridge out VAA will be saved here and used for Bridge In
  let VAA: any = null;

  before(async () => {
    try {

      console.log("Deploying program...");
      await deployProgram("cat_sol20", "Localnet", "/home/ace/.config/solana/id.json").then((stdout) => {
        console.log(stdout);
      });

      // Get the RPC URL for the local cluster.
      const rpc = (provider.connection.rpcEndpoint).toString();
      console.log("RPC: ", rpc);

      // Get the IDL
      const IDL = JSON.parse(fs.readFileSync("./target/idl/cat_sol20.json").toString());

      // Get Program ID
      SPL_CAT_PID = new anchor.web3.PublicKey(anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('./target/deploy/cat_sol20-keypair.json').toString()))).publicKey);

      // Generate the program client from IDL.
      program = program = new anchor.Program(
        IDL,
        SPL_CAT_PID,
        new anchor.AnchorProvider(
          new anchor.web3.Connection(rpc),
          new anchor.Wallet(KEYPAIR),
          {}));

      console.log("Program: ", program.programId.toString());

      tokenMintPDA = PublicKey.findProgramAddressSync([Buffer.from("spl_cat_token")], SPL_CAT_PID)[0];
      tokenMetadataPDA = PublicKey.findProgramAddressSync([Buffer.from("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), tokenMintPDA.toBuffer()], TOKEN_METADATA_PROGRAM_ID)[0];

      const tx = await provider.connection.requestAirdrop(newOwner.publicKey, 10 * LAMPORTS_PER_SOL);
      console.log("Fund AirDrop Transaction: ", tx);
    } catch (e: any) {
      console.log(e);
    }
  });


  describe("Initialization and Minting", () => {
    it("Can Initialize and Create a Mint", async () => {
      try {
        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        // Initial Sequence is 1
        const initial_sequence = Buffer.alloc(8);
        initial_sequence.writeBigUint64LE(BigInt(1));

        const wormhole = getWormholeCpiAccounts(
          CORE_BRIDGE_PID,
          KEYPAIR.publicKey,
          SPL_CAT_PID,
          deriveAddress([Buffer.from("sent"), initial_sequence], SPL_CAT_PID)
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
          owner: KEYPAIR.publicKey,
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
        }).signers([KEYPAIR]);

        const tx = await method.transaction();
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
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

        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          KEYPAIR.publicKey,
        );

        let amount = new anchor.BN("100000000000000000");
        const method = program.methods.mintTokens(amount).accounts({
          owner: KEYPAIR.publicKey,
          ataAuthority: KEYPAIR.publicKey,
          config: configAcc,
          tokenMint: tokenMintPDA,
          tokenUserAta: tokenUserATA,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([KEYPAIR]);

        const tx = await method.transaction();
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
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

    it("Can Shuld Fail mint", async () => {
      try {
        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          KEYPAIR.publicKey,
        );

        let transaction = new anchor.web3.Transaction();
        transaction.add(
          createMintToInstruction(
            tokenMintPDA, // Mint
            tokenUserATA, // Destination
            tokenMintPDA, // Signer (Mint Authority)
            1000000,
          )
        );
        const tx = await anchor.web3.sendAndConfirmTransaction(provider.connection, transaction, [KEYPAIR]);
        expect.fail("Minting should have failed, but it succeeded");
      } catch (e: any) {
        expect(e.message).to.include("Signature verification failed");
      }
    });
  });


  describe("Ownership Transfers", () => {
    it("Can Transfer Config Ownership", async () => {
      const [configAcc, _] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
      ], SPL_CAT_PID);

      const method = program.methods.transferOwnership().accounts({
        owner: KEYPAIR.publicKey,
        newOwner: newOwner.publicKey,
        config: configAcc,
      }).signers([KEYPAIR]);

      const tx = await method.transaction();
      tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
      tx.feePayer = KEYPAIR.publicKey;
      const message = tx.compileMessage();

      const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
      console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

      const simulate = await provider.connection.simulateTransaction(tx);
      console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

      const rpc = await method.rpc();
      console.log("Your transaction signature", rpc);

      // You can assert that the transaction was successful.
      assert.ok(tx, "Transaction failed");
      console.log("Your transaction signature", tx);
    });

    it("Should Fail to Transfer Ownership to Existing Owner", async () => {
      try {
        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const tx = await program.methods.transferOwnership().accounts({
          owner: newOwner.publicKey,
          newOwner: newOwner.publicKey, // Using the same owner here
          config: configAcc,
        }).signers([newOwner]).rpc();

        // If no error was thrown by the previous code, this assertion will fail the test
        expect.fail("Transfer to existing owner should have failed, but it succeeded");
      } catch (e: any) {
        // If an error was thrown, we'll assert that it's the error we expected
        expect(e.message).to.include("AlreadyOwner");
      }
    });


    it("Can Mint Tokens With New Owner", async () => {
      try {

        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          newOwner.publicKey,
        );

        let amount = new anchor.BN("100000000000000000");
        const method = program.methods.mintTokens(amount).accounts({
          owner: newOwner.publicKey,
          ataAuthority: newOwner.publicKey,
          config: configAcc,
          tokenMint: tokenMintPDA,
          tokenUserAta: tokenUserATA,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([newOwner]);

        const tx = await method.transaction();

        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
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
    })
  });

  describe("Registering Chains", () => {

    it("Can Register a chain", async () => {
      try {
        // Registering Ethereum 
        const foreignChainId = Buffer.alloc(2);
        foreignChainId.writeUInt16LE(CHAINS.ethereum);

        const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("foreign_emitter"),
          foreignChainId,
        ], SPL_CAT_PID)

        // Replace this with the Eth Contract
        const ethContractAddress = "0x970e8f18ebfEa0B08810f33a5A40438b9530FBCF";
        let targetEmitterAddress: string | number[] = getEmitterAddressEth(ethContractAddress);
        targetEmitterAddress = Array.from(Buffer.from(targetEmitterAddress, "hex"))

        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const method = program.methods.registerEmitter({
          chain: CHAINS.ethereum,
          address: targetEmitterAddress,
        }).accounts({
          owner: newOwner.publicKey,
          config: configAcc,
          foreignEmitter: emitterAcc,
          systemProgram: anchor.web3.SystemProgram.programId
        })
          .signers([newOwner])

        const tx = await method.transaction();

        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
        const message = tx.compileMessage();

        const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
        console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

        const simulate = await provider.connection.simulateTransaction(tx);
        console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

        const rpc = await method.rpc();
        console.log("Your transaction signature", rpc);

        assert.ok(rpc, "Transaction failed to register a chain");

      } catch (e: any) {
        console.log(e);
        assert.fail(`Unexpected error occurred: ${e.message}`);
      }
    })

    it("Should Fail to Register a chain with an invalid chain", async () => {
      try {
        // Registering Solaan
        const foreignChainId = Buffer.alloc(2);
        foreignChainId.writeUInt16LE(CHAINS.solana);

        const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("foreign_emitter"),
          foreignChainId,
        ], SPL_CAT_PID)

        let targetEmitterAddress: string | number[] = getEmitterAddressSolana(SPL_CAT_PID);
        targetEmitterAddress = Array.from(Buffer.from(targetEmitterAddress, "hex"))

        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const tx = await program.methods.registerEmitter({
          chain: CHAINS.solana,
          address: targetEmitterAddress,
        }).accounts({
          owner: newOwner.publicKey,
          config: configAcc,
          foreignEmitter: emitterAcc,
          systemProgram: anchor.web3.SystemProgram.programId
        })
          .signers([newOwner])
          .rpc();

        console.log("Your transaction signature", tx);
        expect.fail("Chain registration should have failed, but it succeeded");
      } catch (e: any) {
        expect(e.message).to.include("Invalid Chain ID or Zero Address");
      }
    })

  });


  describe("Bridging", () => {
    it("Can Bridge Out", async () => {
      try {
        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        // Make Sure this acc is initialized and has tokens
        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          newOwner.publicKey,
        );

        const foreignChainId = Buffer.alloc(2);
        foreignChainId.writeUInt16LE(CHAINS.ethereum);

        const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("foreign_emitter"),
          foreignChainId,
        ], SPL_CAT_PID)

        // get sequence
        const SequenceTracker = await getProgramSequenceTracker(provider.connection, SPL_CAT_PID, CORE_BRIDGE_PID)
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
          newOwner.publicKey,
          SequenceTracker
        );

        // User's Ethereum address
        let userEthAddress = "0x90F8bf6A479f320ead074411a4B0e7944Ea8c9C1";
        let recipient = Array.from(tryNativeToUint8Array(userEthAddress, "ethereum"));

        // Parameters
        let amount = new anchor.BN("10000000000000000");
        let recipientChain = 2;
        const method = program.methods.bridgeOut({
          amount,
          recipientChain,
          recipient,
        }).accounts({
          owner: newOwner.publicKey,
          ataAuthority: newOwner.publicKey,
          // Token Stuff
          tokenUserAta: tokenUserATA,
          tokenMint: tokenMintPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          // Wormhole Stuff
          wormholeProgram: CORE_BRIDGE_PID,
          foreignEmitter: emitterAcc,
          config: configAcc,
          ...wormholeAccounts,
        }).signers([newOwner])

        const tx = await method.transaction();

        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
        const message = tx.compileMessage();

        const fee = await provider.connection.getFeeForMessage(message, 'confirmed');
        console.log("Transaction Fee: ", fee.value / LAMPORTS_PER_SOL);

        const simulate = await provider.connection.simulateTransaction(tx);
        console.log("Simulated Fee: ", simulate.value.unitsConsumed / LAMPORTS_PER_SOL);

        const rpc = await method.rpc();
        console.log("Your transaction signature", rpc);

        console.log("Your transaction signature", tx);
        await new Promise((r) => setTimeout(r, 3000)); // Wait for tx to be confirmed

        const confirmedTx = await provider.connection.getTransaction(rpc, { commitment: "confirmed", maxSupportedTransactionVersion: 2 });

        const seq = parseSequenceFromLogSolana(confirmedTx)
        const emitterAddr = getEmitterAddressSolana(SPL_CAT_PID.toString()); //same as whDerivedEmitter

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
            tx.partialSign(newOwner);
            return tx;
          },
          CORE_BRIDGE_PID,
          newOwner.publicKey.toString(),
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
          ], SPL_CAT_PID)[0];


        const [configAcc, _] = PublicKey.findProgramAddressSync([
          Buffer.from("config")
        ], SPL_CAT_PID);

        const tokenUserATA = getAssociatedTokenAddressSync(
          tokenMintPDA,
          payload.destUserAddress,
        );

        const foreignChainId = Buffer.alloc(2);
        foreignChainId.writeUInt16LE(payload.sourceTokenChain);

        const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
          Buffer.from("foreign_emitter"),
          foreignChainId,
        ], SPL_CAT_PID)

        const method = program.methods.bridgeIn(Array.from(parsedVAA.hash)).accounts({
          owner: newOwner.publicKey,
          ataAuthority: payload.destUserAddress,
          tokenUserAta: tokenUserATA,
          tokenMint: tokenMintPDA,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          wormholeProgram: CORE_BRIDGE_PID,
          foreignEmitter: emitterAcc,
          posted: postedVAAKey,
          received: recievedKey,
          config: configAcc,
          systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([newOwner]);

        const tx = await method.transaction();
        tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
        tx.feePayer = KEYPAIR.publicKey;
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

});


function getParsedPayload(vaa: Buffer) {
  let offset = 0;

  const amount = vaa.subarray(offset, offset += 32);
  const tokenDecimals = vaa.subarray(offset, offset += 1);
  const sourceTokenAddress = vaa.subarray(offset, offset += 32);
  const sourceUserAddress = vaa.subarray(offset, offset += 32);
  const sourceTokenChain = vaa.subarray(offset, offset += 2);
  const destTokenAddress = vaa.subarray(offset, offset += 32);
  const destUserAddress = vaa.subarray(offset, offset += 32);
  const destTokenChain = vaa.subarray(offset, offset += 2);

  return {
    amount: BigInt(`0x${amount.toString('hex')}`),
    tokenDecimals: tokenDecimals.readUInt8(),
    sourceTokenAddress: sourceTokenAddress.toString('hex'),
    sourceUserAddress: sourceUserAddress.toString('hex'),
    sourceTokenChain: sourceTokenChain.readUInt16BE(),
    destTokenAddress: destTokenAddress.toString('hex'),
    destUserAddress: new PublicKey(destUserAddress),
    destTokenChain: destTokenChain.readUInt16BE()
  }
}