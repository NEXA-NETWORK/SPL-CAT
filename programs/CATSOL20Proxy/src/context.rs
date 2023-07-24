use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{
    error::ErrorFactory,
    cat_struct::CATSOLStructs,
    state::{Config, ForeignEmitter, Received, WormholeEmitter},
};

/// AKA `b"sent"`.
pub const SEED_PREFIX_SENT: &[u8; 4] = b"sent";

pub const SEED_PREFIX_MINT: &'static [u8; 13] = b"cat_spl_token";


#[derive(Accounts)]
/// Context used to initialize program data (i.e. config).
pub struct Initialize<'info> {
    #[account(mut)]
    /// Whoever initializes the config will be the owner of the program. Signer
    /// for creating the [`Config`] account and posting a Wormhole message
    /// indicating that the program is alive.
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        seeds = [Config::SEED_PREFIX],
        bump,
        space = Config::MAXIMUM_SIZE,

    )]
    /// Config account, which saves program data useful for other instructions.
    /// Also saves the payer of the [`initialize`](crate::initialize) instruction
    /// as the program's owner.
    pub config: Box<Account<'info, Config>>,

    // /// CHECK: This is the token mint that will be bridged out
    // pub other_program: AccountInfo<'info>,
    
    // /// SPL Token Mint. Owned by an external program.
    // #[account(
    //     mut,
    //     seeds = [SEED_PREFIX_MINT],
    //     bump, 
    //     seeds::program = other_program.key()
    // )]
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,


    /// Solana SPL token program.
    pub token_program: Program<'info, Token>,
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

#[derive(Accounts)]
#[instruction(chain: u16)]
pub struct RegisterEmitter<'info> {
    #[account(mut)]
    /// Owner of the program set in the [`Config`] account. Signer for creating
    /// the [`ForeignEmitter`] account.
    pub owner: Signer<'info>,

    #[account(
        has_one = owner @ ErrorFactory::OwnerOnly,
        seeds = [Config::SEED_PREFIX],
        bump
    )]
    /// Config account. This program requires that the `owner` specified in the
    /// context equals the pubkey specified in this account. Read-only.
    pub config: Box<Account<'info, Config>>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            &chain.to_le_bytes()[..]
        ],
        bump,
        space = ForeignEmitter::MAXIMUM_SIZE,
    )]
    /// Foreign Emitter account. Create this account if an emitter has not been
    /// registered yet for this Wormhole chain ID. If there already is an
    /// emitter address saved in this account, replace it
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    /// System program.
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BridgeOut<'info> {
    #[account(mut)]
    /// Owner will pay Wormhole fee to post a message and pay for the associated token account.
    pub owner: Signer<'info>,

    #[account(mut)]
    pub ata_authority: Signer<'info>,

    /// Token Mint. The token that is Will be bridged out
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged out
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = ata_authority,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    /// CHECK: Token ATA PDA. The PDA of the ATA that will hold the locked tokens.
    #[account(
        mut,
        seeds = [SEED_PREFIX_MINT, token_user_ata.key().as_ref()],
        bump,
      )]
    pub token_ata_pda: AccountInfo<'info>,

    // Token Mint ATA. Its an Associated Token Account owned by the Program that will hold the locked tokens
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = token_ata_pda,
    )]
    pub token_mint_ata: Account<'info, TokenAccount>,

    // Solana SPL Token Program
    pub token_program: Program<'info, Token>,
    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,

    // --------------------- Wormhole ---------------------

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX],
        bump,
    )]
    /// Config account. Wormhole PDAs specified in the config are checked
    /// against the Wormhole accounts in this context. Read-only.
    pub config: Box<Account<'info, Config>>,

    /// Wormhole program.
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        mut,
        address = config.wormhole.bridge @ ErrorFactory::InvalidWormholeConfig
    )]
    /// Wormhole bridge data. [`wormhole::post_message`] requires this account
    /// be mutable.
    pub wormhole_bridge: Account<'info, wormhole::BridgeData>,

    #[account(
        mut,
        address = config.wormhole.fee_collector @ ErrorFactory::InvalidWormholeFeeCollector
    )]
    /// Wormhole fee collector. [`wormhole::post_message`] requires this
    /// account be mutable.
    pub wormhole_fee_collector: Account<'info, wormhole::FeeCollector>,

    #[account(
        seeds = [WormholeEmitter::SEED_PREFIX],
        bump,
    )]
    /// Program's emitter account. Read-only.
    pub wormhole_emitter: Account<'info, WormholeEmitter>,

    #[account(
        mut,
        address = config.wormhole.sequence @ ErrorFactory::InvalidWormholeSequence
    )]
    /// Emitter's sequence account. [`wormhole::post_message`] requires this
    /// account be mutable.
    pub wormhole_sequence: Account<'info, wormhole::SequenceTracker>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX_SENT,
            &wormhole_sequence.next_value().to_le_bytes()[..]
        ],
        bump,
    )]
    /// CHECK: Wormhole Message. [`wormhole::post_message`] requires this
    /// account be mutable.
    pub wormhole_message: UncheckedAccount<'info>,

    /// System program.
    pub system_program: Program<'info, System>,

    /// Clock sysvar.
    pub clock: Sysvar<'info, Clock>,

    /// Rent sysvar.
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(vaa_hash: [u8; 32])]
pub struct BridgeIn<'info> {
    /// Owner will initialize an account that tracks his own payloads
    #[account(mut)]
    pub owner: Signer<'info>,

    /// ATA Authority. The authority of the ATA that will hold the bridged tokens.
    /// CHECK: This is the authority of the ATA
    #[account(mut)]
    pub ata_authority: UncheckedAccount<'info>,

    /// Token Mint. The token that is bridged in.
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged in.
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = ata_authority,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    /// CHECK: Token ATA PDA. The PDA of the ATA that will hold the locked tokens.
    #[account(
        mut,
        seeds = [SEED_PREFIX_MINT, token_user_ata.key().as_ref()],
        bump,
      )]
    pub token_ata_pda: AccountInfo<'info>,

    // Token Mint ATA. Its an Associated Token Account owned by the Program that will hold the locked tokens
    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_ata_pda,
    )]
    pub token_mint_ata: Account<'info, TokenAccount>,

    // Solana SPL Token Program
    pub token_program: Program<'info, Token>,
    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX],
        bump,
    )]
    /// Config account. Wormhole PDAs specified in the config are checked
    /// against the Wormhole accounts in this context. Read-only.
    pub config: Box<Account<'info, Config>>,

    // Wormhole program.
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        seeds = [
            wormhole::SEED_PREFIX_POSTED_VAA,
            &vaa_hash
        ],
        bump,
        seeds::program = wormhole_program
    )]
    /// Verified Wormhole message account. The Wormhole program verified
    /// signatures and posted the account data here. Read-only.
    pub posted: Account<'info, wormhole::PostedVaa<CATSOLStructs>>,

    #[account(
        init,
        payer = owner,
        seeds = [
            Received::SEED_PREFIX,
            &posted.emitter_chain().to_le_bytes()[..],
            &posted.sequence().to_le_bytes()[..]
        ],
        bump,
        space = Received::MAXIMUM_SIZE
    )]
    pub received: Account<'info, Received>,

    #[account(
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            &posted.emitter_chain().to_le_bytes()[..]
        ],
        bump,
        // constraint = foreign_emitter.verify(posted.emitter_address()) @ ErrorFactory::InvalidForeignEmitter
    )]
    /// Foreign emitter account. The posted message's `emitter_address` must
    /// agree with the one we have registered for this message's `emitter_chain`
    /// (chain ID). Read-only.
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    /// System program.
    pub system_program: Program<'info, System>,
}
