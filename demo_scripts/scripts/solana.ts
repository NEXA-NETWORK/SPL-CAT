import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SplCat } from "../SPL_CAT/target/types/spl_cat";
import { exec } from "child_process";
import { Metaplex } from "@metaplex-foundation/js";
import { TOKEN_METADATA_PROGRAM_ID } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { deriveAddress } from "@certusone/wormhole-sdk/lib/cjs/solana";
import { getAssociatedTokenAddressSync, getOrCreateAssociatedTokenAccount, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token"
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
import base58 from "bs58";
import axios from "axios";
import * as fs from 'fs';

const config = JSON.parse(fs.readFileSync('./xdapp.config.json').toString());
const IDL = JSON.parse(fs.readFileSync("./SPL_CAT/target/idl/spl_cat.json").toString());
const SPL_CAT_PID = new PublicKey("bhp6ce99vHEbpzRjUtpkLQpDQmzbHU5DFBX4pNLVrzb");
const CORE_BRIDGE_PID = "Bridge1p5gheXUvJ6jGWGeCsgPKgnE3YgdGKRVCMY9o";
const tokenMintPDA = PublicKey.findProgramAddressSync([Buffer.from("spl_cat_token")], SPL_CAT_PID)[0];
const tokenMetadataPDA = PublicKey.findProgramAddressSync([Buffer.from("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), tokenMintPDA.toBuffer()], TOKEN_METADATA_PROGRAM_ID)[0];
const KEYPAIR = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync('/home/ace/.config/solana/id.json').toString())));

export async function deploy(src: string) {
    const rpc = config.networks[src]['rpc'];

    //Request Airdrop for saved keypair (because Local Validator probably started with the keypair in ~/.config)
    const connection = new anchor.web3.Connection(rpc);
    await connection.requestAirdrop(KEYPAIR.publicKey, 1e9 * 1000); //request 1000 SOL
    await new Promise((r) => setTimeout(r, 15000)); // wait for the airdrop to go through
    const deployCommand = `cd SPL_CAT && solana config set -u ${rpc} -k ../Keypair.json && anchor build && anchor deploy --program-name spl_cat && exit`
    console.log("Deploying SPL_CAT to Solana...\n");
    exec(
        deployCommand,
        (err, out, errStr) => {
            if (err) {
                throw new Error(err.message);
            }
            if (out) {
                new Promise((r) => setTimeout(r, 15000)) // wait for the chain to recognize the program
                    .then(async () => {
                        //Initalize the Contract
                        const program = new anchor.Program<SplCat>(
                            IDL,
                            SPL_CAT_PID,
                            new anchor.AnchorProvider(
                                new anchor.web3.Connection(rpc),
                                new anchor.Wallet(KEYPAIR),
                                {}));

                        console.log("Initializing SPL_CAT...");
                        const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
                            Buffer.from("config")
                        ], SPL_CAT_PID);

                        const initial_sequence = Buffer.alloc(8);
                        initial_sequence.writeBigUint64LE(BigInt(1));

                        const wormhole = getWormholeCpiAccounts(
                            CORE_BRIDGE_PID,
                            KEYPAIR.publicKey,
                            SPL_CAT_PID,
                            deriveAddress([Buffer.from("sent"), initial_sequence], SPL_CAT_PID)
                        );

                        let max_supply = new anchor.BN("10000000000000000000");
                        // let max_supply = new anchor.BN("18446744073709551615");
                        console.log("Invoking Initialize with:");
                        console.log("Decimals:", 9);
                        console.log("Max Supply:", max_supply.toString());
                        console.log("Name:", "TESTING");
                        console.log("Symbol:", "TST");

                        const tx = await program.methods.initialize(9, max_supply, "TESTING", "TST", "").accounts({
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
                        }).signers([KEYPAIR]).rpc({ skipPreflight: true });
                        console.log("Your transaction signature", tx);
                        console.log("\n");

                        fs.writeFileSync(
                            `./deployinfo/${src}.deploy.json`,
                            JSON.stringify({
                                address: SPL_CAT_PID,
                                tokenAddress: tokenMintPDA.toString(),
                                emitterAddress: wormhole.emitter.toString(),
                                recipientAddress: KEYPAIR.publicKey.toString(),
                                tx: tx,
                                vaas: []
                            }, null, 4)
                        );
                    })
            }
        }
    )
}


export async function registerApp(src: string, target: string) {
    const srcNetwork = config.networks[src];
    const targetNetwork = config.networks[target];
    let srcDeploymentInfo;
    let targetDeploymentInfo;
    let targetEmitter;

    try {
        srcDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${src}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${src} is not deployed yet`);
    }

    try {
        targetDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${target}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${target} is not deployed yet`);
    }

    switch (targetNetwork['type']) {
        case 'evm':
            targetEmitter = getEmitterAddressEth(targetDeploymentInfo['address']);
            break;
        case 'solana':
            targetEmitter = getEmitterAddressSolana(targetDeploymentInfo['address']);
            break;
    }
    if (!targetEmitter) {
        throw new Error(`Target Network ${target} is not supported`);
    }

    // Make sure the Wormhole program is deployed
    const targetChainId = Buffer.alloc(2);
    targetChainId.writeUInt16LE(targetNetwork['wormholeChainId']);

    const program = new anchor.Program<SplCat>(
        IDL,
        SPL_CAT_PID,
        new anchor.AnchorProvider(
            new anchor.web3.Connection(srcNetwork['rpc']),
            new anchor.Wallet(KEYPAIR),
            {}));

    const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        targetChainId,
    ], SPL_CAT_PID)

    const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
    ], SPL_CAT_PID);

    // Decode the string to a Buffer
    let targetEmitterAddress = Array.from(Buffer.from(targetEmitter.slice(2), "hex"))

    const tx = await program.methods.registerEmitter(CHAINS.ethereum, targetEmitterAddress).accounts({
        owner: KEYPAIR.publicKey,
        config: configAcc,
        foreignEmitter: emitterAcc,
        systemProgram: anchor.web3.SystemProgram.programId
    })
        .signers([KEYPAIR])
        .rpc();

    console.log("Your transaction signature", tx);
    console.log(`Successfully Registered ${target} on ${src}`);
    console.log("\n");
}



export async function bridgeOut(src: string, target: string) {
    const srcNetwork = config.networks[src];
    const targetNetwork = config.networks[target];
    let srcDeploymentInfo;
    let targetDeploymentInfo;
    let targetEmitter;

    try {
        srcDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${src}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${src} is not deployed yet`);
    }

    try {
        targetDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${target}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${target} is not deployed yet`);
    }

    const program = new anchor.Program<SplCat>(
        IDL,
        SPL_CAT_PID,
        new anchor.AnchorProvider(
            new anchor.web3.Connection(srcNetwork['rpc']),
            new anchor.Wallet(KEYPAIR),
            {}
        ));

    const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
    ], SPL_CAT_PID);

    const tokenAccountPDA = getAssociatedTokenAddressSync(
        tokenMintPDA,
        KEYPAIR.publicKey,
    );

    // get sequence
    const message = await getProgramSequenceTracker(program.provider.connection, SPL_CAT_PID, CORE_BRIDGE_PID)
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
    let userEthAddress = targetDeploymentInfo.publicKey[0];
    let recipient = Array.from(Buffer.from(userEthAddress.slice(2), "hex"))
    // Pad to 32 bytes
    while (recipient.length < 32) {
        recipient.unshift(0);
    }

    // Parameters
    // let amount = new anchor.BN(5124095576375277); // It causes an Overflow and excees max supply
    let amount = new anchor.BN("5000000000000000000");
    console.log("Amount", amount.toString());
    let recipientChain = 2; // ETH mainnet

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

    const confirmedTx = await program.provider.connection.getTransaction(tx, { commitment: "confirmed", maxSupportedTransactionVersion: 2 });

    const seq = parseSequenceFromLogSolana(confirmedTx!)
    const emitterAddr = getEmitterAddressSolana(SPL_CAT_PID.toString()); //same as whDerivedEmitter

    console.log("Sequence: ", seq);
    console.log("Emitter Address: ", emitterAddr);

    await new Promise((r) => setTimeout(r, 3000)); // Wait for guardian to pick up message
    const restAddress = "http://localhost:7071"

    console.log(
        "Searching for: ",
        `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
    );

    let vaaBytes: any = await axios.get(
        `${restAddress}/v1/signed_vaa/1/${emitterAddr}/${seq}`
    );
    vaaBytes = vaaBytes.data;
    console.log("VAA Bytes: ", vaaBytes);

    if (!vaaBytes['vaaBytes']) {
        throw new Error("VAA not found!");
    }

    if (!srcDeploymentInfo['vaas']) {
        srcDeploymentInfo['vaas'] = [vaaBytes['vaaBytes']]
    } else {
        srcDeploymentInfo['vaas'].push(vaaBytes['vaaBytes'])
    }
    fs.writeFileSync(
        `./deployinfo/${src}.deploy.json`,
        JSON.stringify(srcDeploymentInfo, null, 4)
    );
    console.log("\n");
    return vaaBytes['vaaBytes'];
}

export async function bridgeIn(src: string, target: string, idx: string) {

    const srcNetwork = config.networks[src];
    let srcDeploymentInfo;
    let targetDeploymentInfo;

    try {
        srcDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${src}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${src} is not deployed yet`);
    }

    try {
        targetDeploymentInfo = JSON.parse(fs.readFileSync(`./deployinfo/${target}.deploy.json`).toString());
    } catch (e) {
        throw new Error(`${target} is not deployed yet`);
    }

    const vaa = isNaN(parseInt(idx))
        ? targetDeploymentInfo.vaas.pop()
        : targetDeploymentInfo.vaas[parseInt(idx)];

    const program = new anchor.Program<SplCat>(
        IDL,
        SPL_CAT_PID,
        new anchor.AnchorProvider(
            new anchor.web3.Connection(srcNetwork['rpc']),
            new anchor.Wallet(KEYPAIR),
            {}
        ));
    await postVaaSolanaWithRetry(
        program.provider.connection,
        async (tx) => {
            tx.partialSign(KEYPAIR);
            return tx;
        },
        CORE_BRIDGE_PID,
        KEYPAIR.publicKey.toString(),
        Buffer.from(vaa, "base64"),
        10
    );

    const parsedVAA = parseVaa(Buffer.from(vaa, 'base64'));
    const payload = getParsedPayload(parsedVAA.payload);
    // console.log("Payload", payload);

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


    const [configAcc, configBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("config")
    ], SPL_CAT_PID);

    const tokenAccountPDA = getAssociatedTokenAddressSync(
        tokenMintPDA,
        payload.toAddress,
    );

    // Make sure the Wormhole program is deployed
    const targetChainId = Buffer.alloc(2);
    targetChainId.writeUInt16LE(CHAINS.ethereum);

    const [emitterAcc, emitterBmp] = PublicKey.findProgramAddressSync([
        Buffer.from("foreign_emitter"),
        targetChainId,
    ], SPL_CAT_PID)

    try {
        let balance = await program.provider.connection.getTokenSupply(tokenMintPDA);
        console.log("Balance Before", 0);

        const tx = await program.methods.bridgeIn(Array.from(parsedVAA.hash)).accounts({
            owner: KEYPAIR.publicKey,
            ataAuthority: payload.toAddress,
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

        // Get account balance
        balance = await program.provider.connection.getTokenAccountBalance(tokenAccountPDA);
        console.log("Balance After", balance.value.amount);
        console.log("\n");


    } catch (e) {
        console.log("Error: ", e);
    }
}


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