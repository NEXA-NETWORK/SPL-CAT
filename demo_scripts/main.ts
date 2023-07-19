import { Command } from 'commander';
import * as fs from 'fs';
import * as evm from './scripts/evm';
import * as solana from './scripts/solana';

const config = JSON.parse(fs.readFileSync('./xdapp.config.json').toString())

const program = new Command();
program
    .name('Nexa CAT')
    .description("A reference implementation of a nexa cat.")
    .version("1.0.0");

// $ deploy <chain>
program
    .command('deploy')
    .argument('<network>', 'name of the network in xdapp.config.json to deploy')
    .action(async (network) => {
        if (!config.networks[network]) {
            console.error(`ERROR: ${network} not found in xdapp.config.json`);
            return;
        }
        console.log(`Deploying ${network}...`);

        switch (config.networks[network].type) {
            case "evm":
                await evm.deploy(network);
                break;
            case "solana":
                await solana.deploy(network);
                break;
        }

    });

// $ register-network <source> <target>
program
    .command("register")
    .argument('<source>', 'The network you are registering on.')
    .argument('<target>', 'The foreign network that you want to register.')
    .action(async (src, target) => {
        if (!config.networks[src]) {
            console.error(`ERROR: ${src} not found in xdapp.config.json`);
            return;
        }
        if (!config.networks[target]) {
            console.error(`ERROR: ${target} not found in xdapp.config.json`);
            return;
        }

        console.log(`Registering ${target} on ${src}...`);
        try {
            switch (config.networks[src].type) {
                case 'evm':
                    await evm.registerApp(src, target);
                    break;
                case "solana":
                    await solana.registerApp(src, target);
                    break;
            }

        } catch (e) {
            console.error(`ERROR: ${e}`)
        }
    });

// $ send-msg <source> <msg>
program
    .command('emit')
    .argument('<source>', 'The network you want to emit the message from')
    .argument('<target>', 'The network you want to emit the message to')
    .action(async (src, target) => {
        if (!config.networks[src]) {
            console.error(`ERROR: ${src} not found in xdapp.config.json`);
            return;
        }
        if (!config.networks[target]) {
            console.error(`ERROR: ${target} not found in xdapp.config.json`);
            return;
        }
        console.log(`BridgeOut from ${src} to ${target}...`);
        try {
            switch (config.networks[src].type) {
                case 'evm':
                    await evm.bridgeOut(src, target);
                    break;
                case "solana":
                    await solana.bridgeOut(src, target);
                    break;
            }
        } catch (e) {
            console.error(`ERROR: ${e}`)
        }
    });

// $ relay <source> <target> <vaa#>
program
    .command('relay')
    .argument('<source>', 'The network you want to submit the VAA on')
    .argument('<target>', 'The network you want to submit the VAA from')
    .argument('<vaa#>', 'The index of the VAA in the list of emitted VAAs that you want to submit. Use \'latest\' to submit the latest VAA')
    .action(async (src, target, idx) => {
        if (!config.networks[src]) {
            console.error(`ERROR: ${src} not found in xdapp.config.json`);
            return;
        }
        if (!config.networks[target]) {
            console.error(`ERROR: ${target} not found in xdapp.config.json`);
            return;
        }
        console.log(`BridgeIn from ${target} to ${src}...`);
        try {
            switch (config.networks[src].type) {
                case 'evm':
                    await evm.bridgeIn(src, target, idx);
                    break;
                case "solana":
                    await solana.bridgeIn(src, target, idx);
                    break;
            }

        } catch (e) {
            console.error(`ERROR: ${e}`)
        }
    });

program.parse();