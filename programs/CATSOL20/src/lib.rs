use anchor_lang::prelude::*;

pub use cat_struct::*;
pub use context::*;
pub use error::*;
pub use state::*;
pub use utils::*;
pub use constants::*;

pub mod cat_struct;
pub mod context;
pub mod error;
pub mod state;
pub mod utils;
pub mod constants;

declare_id!("9oMo3tUy3gBYi9FHEDF8YFQBryiUXLqq8wi4Ztsd186Y");

#[program]
pub mod cat_sol20 {
    use super::*;
    use anchor_lang::solana_program::{self, program::invoke};
    use anchor_spl::{
        associated_token,
        token::{burn, mint_to, Burn, MintTo}
    };
    use mpl_token_metadata::instruction::create_metadata_accounts_v3;
    use wormhole_anchor_sdk::wormhole;

    pub fn initialize(
        ctx: Context<Initialize>,
        _decimals: u8,
        max_supply: u64,
        name: String,
        symbol: String,
        uri: String,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.owner.key();

        {
            let wormhole = &mut config.wormhole;
            wormhole.bridge = ctx.accounts.wormhole_bridge.key();
            wormhole.fee_collector = ctx.accounts.wormhole_fee_collector.key();
            wormhole.sequence = ctx.accounts.wormhole_sequence.key();
        }

        // Set default values for posting Wormhole messages.
        //
        // Zero means no batching.
        config.batch_id = 0;

        // Anchor IDL default coder cannot handle wormhole::Finality enum,
        // so this value is stored as u8.
        config.finality = wormhole::Finality::Confirmed as u8;

        // Set the Max and Minted Supply
        config.max_supply = max_supply;
        config.minted_supply = ctx.accounts.token_mint.supply;

        // Create Metadata for the tokens.
        {
            let create_metadata_account_ix = create_metadata_accounts_v3(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.metadata_account.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.owner.key(),
                ctx.accounts.owner.key(),
                ctx.accounts.owner.key(),
                name,
                symbol,
                uri,
                None,
                0,
                true,
                true,
                None,
                None,
                None,
            );
            match invoke(
                &create_metadata_account_ix,
                &[
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.metadata_account.to_account_info(),
                    ctx.accounts.token_mint.to_account_info(),
                    ctx.accounts.metadata_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            ) {
                Ok(_) => {}
                Err(e) => {
                    msg!("Error Creating Metadata: {:?}", e);
                    return Err(e.into());
                }
            }
        }

        ctx.accounts.wormhole_emitter.bump = *ctx
            .bumps
            .get("wormhole_emitter")
            .ok_or(ErrorFactory::BumpNotFound)?;

        // Now We will send a message to initialize the Sequence Tracker for future messages
        {
            // Pay the Fee
            let fee = ctx.accounts.wormhole_bridge.fee();
            if fee > 0 {
                match solana_program::program::invoke(
                    &solana_program::system_instruction::transfer(
                        &ctx.accounts.owner.key(),
                        &ctx.accounts.wormhole_fee_collector.key(),
                        fee,
                    ),
                    &ctx.accounts.to_account_infos(),
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            let wormhole_emitter = &ctx.accounts.wormhole_emitter;
            let config = &ctx.accounts.config;

            let mut payload: Vec<u8> = Vec::new();
            CATSOLStructs::serialize(
                &&CATSOLStructs::Alive {
                    program_id: *ctx.program_id,
                },
                &mut payload,
            )?;

            match wormhole::post_message(
                CpiContext::new_with_signer(
                    ctx.accounts.wormhole_program.to_account_info(),
                    wormhole::PostMessage {
                        config: ctx.accounts.wormhole_bridge.to_account_info(),
                        message: ctx.accounts.wormhole_message.to_account_info(),
                        emitter: wormhole_emitter.to_account_info(),
                        sequence: ctx.accounts.wormhole_sequence.to_account_info(),
                        payer: ctx.accounts.owner.to_account_info(),
                        fee_collector: ctx.accounts.wormhole_fee_collector.to_account_info(),
                        clock: ctx.accounts.clock.to_account_info(),
                        rent: ctx.accounts.rent.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                    },
                    &[
                        &[
                            SEED_PREFIX_SENT,
                            &wormhole::INITIAL_SEQUENCE.to_le_bytes()[..],
                            &[*ctx
                                .bumps
                                .get("wormhole_message")
                                .ok_or(ErrorFactory::BumpNotFound)?],
                        ],
                        &[wormhole::SEED_PREFIX_EMITTER, &[wormhole_emitter.bump]],
                    ],
                ),
                config.batch_id,
                payload,
                config.finality.into(),
            ) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Check if the amount doesn't exceed the max supply
        if amount + config.minted_supply >= config.max_supply {
            return Err(ErrorFactory::IvalidMintAmount.into());
        }

        // Mint the tokens
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = MintTo {
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.token_user_ata.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        match mint_to(cpi_ctx, amount) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }
        // Update the Minted Supply
        config.minted_supply += amount;

        Ok(())
    }

    pub fn register_emitter(
        ctx: Context<RegisterEmitter>,
        chain: u16,
        address: [u8; 32],
    ) -> Result<()> {
        // Foreign emitter cannot share the same Wormhole Chain ID as the
        // Solana Wormhole program's. And cannot register a zero address.
        require!(
            chain > 0 && chain != wormhole::CHAIN_ID_SOLANA && !address.iter().all(|&x| x == 0),
            ErrorFactory::InvalidForeignEmitter,
        );

        // Save the emitter info into the ForeignEmitter account.
        let emitter = &mut ctx.accounts.foreign_emitter;
        emitter.chain = chain;
        emitter.address = address;

        // Done.
        Ok(())
    }

    pub fn bridge_out(
        ctx: Context<BridgeOut>,
        amount: u64,
        recipient_chain: u16,
        recipient: [u8; 32],
    ) -> Result<()> {
        // Pay the Fee
        let fee = ctx.accounts.wormhole_bridge.fee();
        if fee > 0 {
            solana_program::program::invoke(
                &solana_program::system_instruction::transfer(
                    &ctx.accounts.owner.key(),
                    &ctx.accounts.wormhole_fee_collector.key(),
                    fee,
                ),
                &ctx.accounts.to_account_infos(),
            )?;
        }

        // Normalize the amount to a Standard 8 decimals
        let decimals = ctx.accounts.token_mint.decimals;
        let normalized_amount = utils_cat::normalize_amount(amount, decimals);

        // Burn the tokens
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Burn {
            mint: ctx.accounts.token_mint.to_account_info(),
            from: ctx.accounts.token_user_ata.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        match burn(cpi_ctx, normalized_amount) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }

        // Create the payload
        let payload = CrossChainStruct {
            amount: U256::from(amount),
            token_address: ctx.accounts.token_user_ata.key().to_bytes(),
            token_chain: wormhole::CHAIN_ID_SOLANA,
            to_address: recipient,
            to_chain: recipient_chain,
            token_decimals: ctx.accounts.token_mint.decimals,
        };

        // Serialize the payload
        let cat_sol_struct = CATSOLStructs::CrossChainPayload { payload };
        let mut encoded_payload: Vec<u8> = Vec::new();
        cat_sol_struct.serialize(&mut encoded_payload)?;

        
        let wormhole_emitter = &ctx.accounts.wormhole_emitter;
        let config = &ctx.accounts.config;

        // Invoke `wormhole::post_message`.
        //
        // `wormhole::post_message` requires two signers: one for the emitter
        // and another for the wormhole message data. Both of these accounts
        // are owned by this program.
        match wormhole::post_message(
            CpiContext::new_with_signer(
                ctx.accounts.wormhole_program.to_account_info(),
                wormhole::PostMessage {
                    config: ctx.accounts.wormhole_bridge.to_account_info(),
                    message: ctx.accounts.wormhole_message.to_account_info(),
                    emitter: wormhole_emitter.to_account_info(),
                    sequence: ctx.accounts.wormhole_sequence.to_account_info(),
                    payer: ctx.accounts.owner.to_account_info(),
                    fee_collector: ctx.accounts.wormhole_fee_collector.to_account_info(),
                    clock: ctx.accounts.clock.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                },
                &[
                    &[
                        SEED_PREFIX_SENT,
                        &ctx.accounts.wormhole_sequence.next_value().to_le_bytes()[..],
                        &[*ctx
                            .bumps
                            .get("wormhole_message")
                            .ok_or(ErrorFactory::BumpNotFound)?],
                    ],
                    &[wormhole::SEED_PREFIX_EMITTER, &[wormhole_emitter.bump]],
                ],
            ),
            config.batch_id,
            encoded_payload,
            config.finality.into(),
        ) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }

        // Done.
        Ok(())
    }

    pub fn bridge_in(ctx: Context<BridgeIn>, vaa_hash: [u8; 32]) -> Result<()> {
        let config = &mut ctx.accounts.config;
        let posted_message = &ctx.accounts.posted;

        if let CATSOLStructs::CrossChainPayload { payload } = posted_message.data() {
            let ata_address = associated_token::get_associated_token_address(
                &Pubkey::from(payload.to_address),
                &ctx.accounts.token_mint.key(),
            );

            // Check if the ATA address is the same as the one in the payload
            require_keys_eq!(
                ata_address,
                ctx.accounts.token_user_ata.key(),
                ErrorFactory::MisMatchdATAAddress
            );

            // Normalize the amount by converting it back from the standard 8 decimals to the token's decimals
            let amount_u64: u64 = payload.amount.into();
            let decimals = ctx.accounts.token_mint.decimals;
            let normalized_amount = utils_cat::denormalize_amount(
                amount_u64,
                decimals,
            );

            // Check if the amount doesn't exceed the max supply
            if normalized_amount + config.minted_supply > config.max_supply {
                return Err(ErrorFactory::IvalidMintAmount.into());
            }

            // Mint the tokens
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.token_user_ata.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

            match mint_to(cpi_ctx, normalized_amount) {
                Ok(_) => {}
                Err(e) => {
                    return Err(e);
                }
            }
            config.minted_supply += normalized_amount;

            // Serialize the payload to save it
            let mut serialized_payload: Vec<u8> = Vec::new();
            CATSOLStructs::CrossChainPayload {
                payload: payload.clone(),
            }
            .serialize(&mut serialized_payload)?;

            //Save batch ID, keccak256 hash and message payload.
            let received = &mut ctx.accounts.received;
            received.batch_id = posted_message.batch_id();
            received.payload = serialized_payload;
            received.wormhole_message_hash = vaa_hash;

            // Done
            Ok(())
        } else {
            Err(ErrorFactory::InvalidMessage.into())
        }
    }
}
