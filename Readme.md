# Documentation

# SPL-CAT

## **Environment Setup**

First of all make sure that the `Cargo.lock` file exists in the project directory. Do not change it or build it from scratch. The `Cargo.lock` file is used to lock the versions of the dependencies. If you build it from scratch, you may get errors due to incompatible versions of the dependencies.

1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Install Solana CLI

```bash
sh -c "$(curl -sSfL https://release.solana.com/v1.14.18/install)"
```

3. Install AVM CLI

```bash
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
```

On Linux systems you may need to install additional dependencies if cargo install fails. E.g. on Ubuntu:

```bash
sudo apt-get update && sudo apt-get upgrade && sudo apt-get install -y pkg-config build-essential libudev-dev libssl-dev
```

4. Install Anchor
  
```bash
avm install 0.28.0
avm use 0.28.0
```

## **Introduction**

This is a Solana smart contract for a token bridge application named **`CATSOL`**. The purpose of the contract is to mint and transfer tokens across different blockchains using the Wormhole bridge protocol. The contract is written in Rust and uses the Anchor framework.

The contract has several components:

1. Initialization
2. Register Emitter
3. Bridge-Out
4. Bridge-In

Each component consists of different instructions and associated accounts. These instructions and accounts will be discussed in detail.

## **Project Setup and Testing**

---

```bash
git clone https://github.com/NEXA-NETWORK/SPL-CAT
```

To run Local tests, we'll use the wormhole local validator.

```bash
git clone https://github.com/hmzakhalid/wormhole-local-validator
cd wormhole-local-validator
npm install
npm run evm
npm run solana
npm run wormhole
```

After that’s done, you should have a local validator running on your machine. Now, we can run the tests.

```bash
anchor test --skip-local-validator
```

## Contract

---

## **Instructions**

### **Initialize**

This is the most important instruction, the rest of them are pretty self-explanatory. This instruction sets up the initial state of the contract, including:

1. Token Mint Account.
2. Metadata Account.
3. Wormhole Configurations (Bridge, Fee Collector, Sequence Tracker).
4. Token metadata (Name, Symbol, URI).
5. Maximum token supply.

**`Initialize`** requires the following accounts:

1. **`owner`**: The account that signs and pays for the transaction.
2. **`config`**: Configuration account. It holds details about the minted tokens and Wormhole configurations. This account is a PDA with a seed prefix of **`config`**.
3. **`token_mint`**: Account representing the mint for the token. This account is a PDA with a seed prefix of `**spl_cat_token**`
4. **`token_program`**: The SPL Token program.
5. **`metadata_account`**: Account to hold the metadata of the token. This account is a PDA with a seed prefix of `**metadata**`the `**metadata_program**` and the `**token_mint**` PDA.
6. **`metadata_program`**: The Metadata program.
7. **`system_program`**: The System program.

Now let’s come to the wormhole related accounts. We can get all of these accounts using the wormhole Typescript SDK function `**getWormholeCpiAccounts` .** The function returns a list of wormhole account PDAs required to initialize the contract.

1. **`wormhole_bridge`**: Account representing the wormhole bridge.
2. **`wormhole_fee_collector`**: Account representing the fee collector of the wormhole.
3. **`wormhole_emitter`**: Account representing the wormhole emitter.
4. **`wormhole_sequence`**: Account representing the wormhole sequence tracker.
5. **`wormhole_message`**: Account representing the wormhole message.
6. **`wormhole_program`**: The Wormhole program.
7. **`clock`**: The Clock system variable.
8. **`rent`**: The Rent system variable.

### **Register Emitter**

This instruction is used to register a new foreign emitter. It requires the **`owner`**, the **`config`** accounts, and the **`foreign_emitter`** account to be initialized if it doesn't exist. The foreign emitter's chain and address are passed as arguments to this instruction. It is necessary for a chain to be registered first if a user wants to bridge token in and out.

### **Bridge-Out**

This instruction transfers tokens from Solana to a different blockchain. It burns tokens from the sender's account and emits a message through the Wormhole bridge. The amount of tokens, the recipient chain and recipient's address are passed as arguments to this instruction.

### **Bridge-In**

This instruction transfers tokens from a different blockchain to Solana. It verifies the posted VAA, mints new tokens to the recipient's account, and marks the VAA as executed. The hash of the VAA is passed as an argument to this instruction.

Bridge-In in particular has a different way of operation in Solana than other chains. Since Solana requires all accounts that are to be modified on chain; to be pass from the client. We need to parse the payload first on the client and get the receiver's address. We then create an Associated Token Account for that address if it doesn’t exist and pass it in as the account required for holding the tokens. To make this process more secure. I’ve added a check on chain that verifies that the ATA sent from the client is indeed derived from the address that we got in the Payload. 
Here’s the code snippet for that:

**Off-Chain:**

```tsx
// Parse the VAA
const parsedVAA = parseVaa(Buffer.from(VAA, 'base64'));
// Decode the Payload
const payload = getParsedPayload(parsedVAA.payload);
// Get the ATA to pass into the instruction
const tokenAccountPDA = getAssociatedTokenAddressSync(
  tokenMintPDA,
  payload.toAddress,
);
```

**On-Chain:**

```rust
let ata_address = associated_token::get_associated_token_address(
    &Pubkey::from(payload.to_address),
    &ctx.accounts.token_mint.key(),
);

// Check if the ATA address is valid
require_keys_eq!(
    ata_address,
    ctx.accounts.token_account.key(),
    ErrorFactory::InvalidATAAddress
);
```