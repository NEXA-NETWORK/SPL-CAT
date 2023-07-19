import * as fs from 'fs';
import { exec } from "child_process";
import { getEmitterAddressEth, getEmitterAddressSolana, parseSequenceFromLogEth } from '@certusone/wormhole-sdk';
import * as ethers from 'ethers';
import fetch from 'node-fetch';
import bs58 from 'bs58';


const config = JSON.parse(fs.readFileSync('./xdapp.config.json').toString());
const name = "TestToken";
const symbol = "TST";
const decimals = 9;
const maxSupply = "10000000000000000000";
const wormholeChainId = "2";
const wormholeCoreContract = "0xC89Ce4735882C9F0f0FE26686c53074E09B0D550";
const SOLANA_CHAIN_ID = 1;

const nowTime = Math.floor(new Date().getTime() / 1000);
const validTime = nowTime + 300;

async function getNetwork() {
    const provider = new ethers.providers.JsonRpcProvider("http://localhost:8545");
    const owner = provider.getSigner(0);
    const otherAccount = provider.getSigner(1);
    return {
        provider,
        owner,
        otherAccount
    }
}

async function makeSignature(custodian: any, validTill: any, signer: ethers.ethers.providers.JsonRpcSigner) {
    let messageHash = ethers.utils.solidityKeccak256(
        ["address", "uint256"],
        [custodian, validTill]
    );

    let messageHashBinary = ethers.utils.arrayify(messageHash);
    let signature = await signer.signMessage(messageHashBinary);

    return { custodian, validTill, signature };
}

export async function deploy(chain: string) {

    exec(
        `cd CAT && npx hardhat run --network ${chain} scripts/deployEVM.ts`,
        (err, out, errStr) => {
            if (err) {
                throw new Error(err.message);
            }
            if (errStr) {
                throw new Error(errStr);
            }
            if (out) {
                console.log(out);
                console.log("\n");
            }
        }
    );
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
        throw new Error(`No emitter address found for ${target}`);
    }

    const signer = new ethers.Wallet(srcDeploymentInfo.privateKeys[0]).connect(
        new ethers.providers.JsonRpcProvider(srcDeploymentInfo.url)
    );

    const CATERC20Instance = new ethers.Contract(
        srcDeploymentInfo.address,
        JSON.parse(
            fs
                .readFileSync(
                    "./CAT/artifacts/contracts/ERC20/CATERC20.sol/CATERC20.json"
                )
                .toString()
        ).abi,
        signer
    );


    const { owner, otherAccount } = await getNetwork();
    const { custodian, validTill, signature } = await makeSignature(
        await otherAccount.getAddress(),
        validTime,
        owner
    );

    const SignatureVerification = [custodian, validTill, signature];

    const SPLTokenAddress = bs58.decode(targetDeploymentInfo.emitterAddress);

    const tx = await CATERC20Instance.registerChain(
        SOLANA_CHAIN_ID,
        SPLTokenAddress,
        SignatureVerification
    ).then((tx: any) => tx.wait());


    const network = await CATERC20Instance.connect(owner).tokenContracts(1);
    console.log("Registered Emitter: ", network);
    console.log(`Successfully Registered ${target} on ${src}`);
    console.log("\n")
}


export async function bridgeOut(src: string, target: string) {
    const srcNetwork = config.networks[src];
    const targetNetwork = config.networks[target];
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


    const signer = new ethers.Wallet(srcDeploymentInfo.privateKeys[0]).connect(
        new ethers.providers.JsonRpcProvider(srcDeploymentInfo.url)
    );

    const CATERC20Instance = new ethers.Contract(
        srcDeploymentInfo.address,
        JSON.parse(
            fs
                .readFileSync(
                    "./CAT/artifacts/contracts/ERC20/CATERC20.sol/CATERC20.json"
                )
                .toString()
        ).abi,
        signer
    );

    const amountToMint = "5000000000000000000";
    const ReceipientAddress = bs58.decode(targetDeploymentInfo.recipientAddress);
    const nonce = 0;

    await CATERC20Instance.mint(signer.address, amountToMint).then((tx: any) => tx.wait());
    const tx = await CATERC20Instance.bridgeOut(
        amountToMint,
        targetNetwork.wormholeChainId,
        ReceipientAddress,
        nonce
    ).then((tx: any) => tx.wait());

    // await tx.wait();
    // console.log(tx);
    const emitterAddr = getEmitterAddressEth(CATERC20Instance.address);
    const seq = parseSequenceFromLogEth(tx, wormholeCoreContract);
    console.log("Sequece Number: ", seq);
    console.log("Emitter Address: ", emitterAddr);

    await new Promise((r) => setTimeout(r, 3000)); // Wait for guardian to pick up message

    await new Promise((r) => setTimeout(r, 5000)); //wait for Guardian to pick up message
    console.log(
        "Searching for: ",
        `${config.wormhole.restAddress}/v1/signed_vaa/${srcNetwork.wormholeChainId}/${emitterAddr}/${seq}`
    );
    const vaaBytes = await (
        await fetch(
            `${config.wormhole.restAddress}/v1/signed_vaa/${srcNetwork.wormholeChainId}/${emitterAddr}/${seq}`
        )
    ).json();

    console.log("VAA: ", vaaBytes['vaaBytes']);

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

    const vaa = Buffer.from((isNaN(parseInt(idx))
        ? targetDeploymentInfo.vaas.pop()
        : targetDeploymentInfo.vaas[parseInt(idx)]), "base64")

    const signer = new ethers.Wallet(srcDeploymentInfo.privateKeys[0]).connect(
        new ethers.providers.JsonRpcProvider(srcDeploymentInfo.url)
    );

    const CATERC20Instance = new ethers.Contract(
        srcDeploymentInfo.address,
        JSON.parse(
            fs
                .readFileSync(
                    "./CAT/artifacts/contracts/ERC20/CATERC20.sol/CATERC20.json"
                )
                .toString()
        ).abi,
        signer
    );

    let balance = await CATERC20Instance.balanceOf(signer.address);
    console.log("Balance Before Bridge-In", balance.toString())
    const tx = await CATERC20Instance.bridgeIn(vaa).then((tx: any) => tx.wait());
    balance = await CATERC20Instance.balanceOf(signer.address);
    console.log("Balance After Bridge-In", balance.toString())
}
