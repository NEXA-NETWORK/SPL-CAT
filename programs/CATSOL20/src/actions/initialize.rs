use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    token::{Mint, Token},
    metadata::Metadata,
};

use crate::{
    constants::*,
    error::ErrorFactory,
    cat_struct::CATSOLStructs,
    state::{Config, WormholeEmitter}
};

use anchor_lang::solana_program::{self, program::invoke_signed};
use mpl_token_metadata::instruction::create_metadata_accounts_v3;

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct InitializeParams {
    pub decimals: u8,
    pub max_supply: u64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

#[derive(Accounts)]
#[instruction(params: InitializeParams)]
/// Context used to initialize program data (i.e. config).
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: The user account we're initializing for. Required for creating PDAs
    pub user: AccountInfo<'info>,

    #[account(
        init,
        payer = owner,
        seeds = [Config::SEED_PREFIX, user.key().as_ref()],
        bump,
        space = Config::MAXIMUM_SIZE,

    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init, 
        seeds = [SEED_PREFIX_MINT, user.key().as_ref()],
        bump,
        payer = owner,
        mint::decimals = params.decimals,
        mint::authority = token_mint.key(),
    )]
    pub token_mint: Account<'info, Mint>,

    ///CHECK:
    #[account(
        mut,
        seeds = [
            b"metadata",
            mpl_token_metadata::id().as_ref(),
            token_mint.key().as_ref(),
        ],
        bump,
        seeds::program = mpl_token_metadata::id()  
    )]
    pub metadata_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        mut,
        seeds = [wormhole::BridgeData::SEED_PREFIX],
        bump,
        seeds::program = wormhole_program,
    )]
    pub wormhole_bridge: Account<'info, wormhole::BridgeData>,

    #[account(
        mut,
        seeds = [wormhole::FeeCollector::SEED_PREFIX],
        bump,
        seeds::program = wormhole_program
    )]
    pub wormhole_fee_collector: Account<'info, wormhole::FeeCollector>,

    #[account(
        init,
        payer = owner,
        seeds = [WormholeEmitter::SEED_PREFIX, token_mint.key().as_ref()],
        bump,
        space = WormholeEmitter::MAXIMUM_SIZE
    )]
    pub wormhole_emitter: Account<'info, WormholeEmitter>,

    #[account(
        mut,
        seeds = [
            wormhole::SequenceTracker::SEED_PREFIX,
            wormhole_emitter.key().as_ref()
        ],
        bump,
        seeds::program = wormhole_program
    )]
    ///CHECK: Its an UncheckedAccount because wormhole will be the one to initialize it.
    pub wormhole_sequence: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX_SENT,
            wormhole_emitter.key().as_ref(),
            &wormhole::INITIAL_SEQUENCE.to_le_bytes()[..]
        ],
        bump,
    )]
    ///CHECK: 
    pub wormhole_message: UncheckedAccount<'info>,

    pub clock: Sysvar<'info, Clock>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}


impl Initialize<'_> {
    pub fn initialize(
        ctx: Context<Initialize>,
        params: &InitializeParams,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.owner.key();

        {
            let wormhole = &mut config.wormhole;
            wormhole.bridge = ctx.accounts.wormhole_bridge.key();
            wormhole.fee_collector = ctx.accounts.wormhole_fee_collector.key();
            wormhole.sequence = ctx.accounts.wormhole_sequence.key();
            msg!("Wormhole: {:?}", wormhole);
        }

        // Set default values for posting Wormhole messages.
        // Zero means no batching.
        config.batch_id = 0;

        // Anchor IDL default coder cannot handle wormhole::Finality enum,
        // so this value is stored as u8.
        config.finality = wormhole::Finality::Confirmed as u8;

        // Set the Max and Minted Supply
        config.max_supply = params.max_supply;
        config.minted_supply = ctx.accounts.token_mint.supply;

        // Create Metadata for the tokens.
        {
            let create_metadata_account_ix = create_metadata_accounts_v3(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.metadata_account.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.owner.key(),
                ctx.accounts.token_mint.key(),
                params.name.clone(),
                params.symbol.clone(),
                params.uri.clone(),
                None,
                0,
                true,
                true,
                None,
                None,
                None,
            );

            let bump = *ctx
            .bumps
            .get("token_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

            let user_key = &ctx.accounts.user;

            let metadata_signer_seeds = &[
                b"spl_cat_token".as_ref(),
                user_key.key.as_ref(),
                &[bump],
            ];


            invoke_signed(
                &create_metadata_account_ix,
                &[
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.metadata_account.to_account_info(),
                    ctx.accounts.token_mint.to_account_info(),
                    ctx.accounts.metadata_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[metadata_signer_seeds],
            )?;
        }

        ctx.accounts.wormhole_emitter.bump = *ctx.bumps.get("wormhole_emitter").ok_or(ErrorFactory::BumpNotFound)?;

        // Now We will send a message to initialize the Sequence Tracker for future messages
        {
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
            let wormhole_emitter = &ctx.accounts.wormhole_emitter;
            let token_mint = &ctx.accounts.token_mint;
            let config = &ctx.accounts.config;

            let mut payload: Vec<u8> = Vec::new();
            CATSOLStructs::serialize(
                &&CATSOLStructs::Alive {
                    program_id: *ctx.program_id,
                },
                &mut payload,
            )?;

            wormhole::post_message(
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
                            wormhole_emitter.key().as_ref(),
                            &wormhole::INITIAL_SEQUENCE.to_le_bytes()[..],
                            &[*ctx
                                .bumps
                                .get("wormhole_message")
                                .ok_or(ErrorFactory::BumpNotFound)?],
                        ],
                        &[wormhole::SEED_PREFIX_EMITTER, token_mint.key().as_ref(), &[wormhole_emitter.bump]],
                    ],
                ),
                config.batch_id,
                payload,
                config.finality.into(),
            )?;
        }

        Ok(())
    }
}
       