## Project Setup and Testing

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
After thats done, you should have a local validator running on your machine.
Now, we can run the tests.
```bash
anchor test --skip-local-validator
```