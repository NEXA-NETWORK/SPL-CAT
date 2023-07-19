# Deploy Contracts
ts-node main.ts deploy sol 
ts-node main.ts deploy evm

# Register Networks
ts-node main.ts register sol evm
ts-node main.ts register evm sol

# EVM to SOLANA BridgeOut
ts-node main.ts emit evm sol

# SOLANA from EVM BridgeIn
ts-node main.ts relay sol evm latest

# SOLANA to EVM BridgeOut
ts-node main.ts emit sol evm

# EVM from SOLANA BridgeIn
ts-node main.ts relay evm sol latest