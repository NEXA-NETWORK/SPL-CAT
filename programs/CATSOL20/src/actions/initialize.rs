use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    token::{Mint, Token},
    metadata::Metadata,
};

use crate::{
    constants::*,
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
    /// Owner will initialize an account that tracks his own payloads
    #[account(mut)]
    pub owner: Signer<'info>,

    // Config Account is used to store the native token on initialization
    // The owner of the config account is basically the owner of the program
    // They can add foreign emitters and mint tokens
    #[account(
        init,
        payer = owner,
        seeds = [Config::SEED_PREFIX],
        bump,
        space = Config::MAXIMUM_SIZE,

    )]
    pub config: Box<Account<'info, Config>>,


    /// Token Mint Account. The token that is Will be bridged out
    #[account(
        init, 
        seeds = [SEED_PREFIX_MINT],
        bump,
        payer = owner,
        mint::decimals = params.decimals,
        mint::authority = token_mint.key(),
    )]
    pub token_mint: Account<'info, Mint>,

    /// CHECK: Metadata account for the token.
    /// Stores the token's name, symbol, logo, and other metadata.
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
    pub metadata_account: UncheckedAccount<'info>,

    /// Token Program.
    pub token_program: Program<'info, Token>,

    /// Metadata program.
    pub metadata_program: Program<'info, Metadata>,

    /// Wormhole program.
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        mut,
        seeds = [wormhole::BridgeData::SEED_PREFIX],
        bump,
        seeds::program = wormhole_program,
    )]
    /// Wormhole bridge data account (a.k.a. its config).
    /// [`wormhole::post_message`] requires this account be mutable.
    pub wormhole_bridge: Account<'info, wormhole::BridgeData>,

    #[account(
        mut,
        seeds = [wormhole::FeeCollector::SEED_PREFIX],
        bump,
        seeds::program = wormhole_program
    )]
    /// Wormhole fee collector account, which requires lamports before the
    /// program can post a message (if there is a fee).
    /// [`wormhole::post_message`] requires this account be mutable.
    pub wormhole_fee_collector: Account<'info, wormhole::FeeCollector>,

    #[account(
        init,
        payer = owner,
        seeds = [WormholeEmitter::SEED_PREFIX],
        bump,
        space = WormholeEmitter::MAXIMUM_SIZE
    )]
    /// This program's emitter account. We create this account in the
    /// [`initialize`](crate::initialize) instruction, but
    /// [`wormhole::post_message`] only needs it to be read-only.
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
    /// CHECK: Emitter's sequence account. This is not created until the first
    /// message is posted, so it needs to be an [UncheckedAccount] for the
    /// [`initialize`](crate::initialize) instruction.
    /// [`wormhole::post_message`] requires this account be mutable.
    pub wormhole_sequence: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX_SENT,
            &wormhole::INITIAL_SEQUENCE.to_le_bytes()[..]
        ],
        bump,
    )]
    /// CHECK: Wormhole message account. The Wormhole program writes to this
    /// account, which requires this program's signature.
    /// [`wormhole::post_message`] requires this account be mutable.
    pub wormhole_message: UncheckedAccount<'info>,

    /// Clock sysvar.
    pub clock: Sysvar<'info, Clock>,

    /// Rent sysvar.
    pub rent: Sysvar<'info, Rent>,

    /// System program.
    pub system_program: Program<'info, System>,
}


impl Initialize<'_> {
    pub fn initialize(
        ctx: Context<Initialize>,
        params: &InitializeParams,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Set the owner of the config (effectively the owner of the program).
        config.owner = ctx.accounts.owner.key();

        // Set the Max and Minted Supply
        config.max_supply = params.max_supply;
        config.minted_supply = ctx.accounts.token_mint.supply;

        // Set Wormhole related addresses.
        {
            let wormhole = &mut config.wormhole;

            // wormhole::BridgeData (Wormhole's program data).
            wormhole.bridge = ctx.accounts.wormhole_bridge.key();

            // wormhole::FeeCollector (lamports collector for posting
            // messages).
            wormhole.fee_collector = ctx.accounts.wormhole_fee_collector.key();

            // wormhole::SequenceTracker (tracks # of messages posted by this
            // program).
            wormhole.sequence = ctx.accounts.wormhole_sequence.key();
        }

        // Set default values for posting Wormhole messages.
        //
        // Zero means no batching.
        config.batch_id = 0;

        // Anchor IDL default coder cannot handle wormhole::Finality enum,
        // so this value is stored as u8.
        config.finality = wormhole::Finality::Confirmed as u8;


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

            let bump = ctx.bumps.token_mint;

            let metadata_signer_seeds = &[
                b"spl_cat_token".as_ref(),
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

        // Storing the BumpSeed for the Wormhole Emitter
        ctx.accounts.wormhole_emitter.bump = ctx.bumps.wormhole_emitter;

        // Now We will send a message to initialize the Sequence Tracker for future messages
        // by posting a message to the Wormhole program.
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
                            &wormhole::INITIAL_SEQUENCE.to_le_bytes()[..],
                            &[ctx.bumps.wormhole_message],
                        ],
                        &[wormhole::SEED_PREFIX_EMITTER, &[wormhole_emitter.bump]],
                    ],
                ),
                config.batch_id,
                payload,
                config.finality.into(),
            )?;
        }

        // done
        Ok(())
    }
}
       