use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::config::Config;


pub const SEED_PREFIX_MINT: &'static [u8; 10] = b"test_token";


#[derive(Accounts)]
#[instruction(_decimals: u8)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: This is the authority of the ATA
    #[account(mut)]
    pub ata_authority: AccountInfo<'info>,

    #[account(
        init,
        payer = owner,
        seeds = [Config::SEED_PREFIX],
        bump,
        space = Config::MAXIMUM_SIZE,

    )]
    pub config: Box<Account<'info, Config>>,


    #[account(
        init, 
        seeds = [SEED_PREFIX_MINT],
        bump,
        payer = owner,
        mint::decimals = _decimals,
        mint::authority = owner,
    )]
    pub token_mint: Account<'info, Mint>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged in.
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = ata_authority,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    /// Solana SPL token program.
    pub token_program: Program<'info, Token>,
    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// System program.
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    /// ATA Authority. The authority of the ATA that will hold the bridged tokens.
    /// CHECK: This is the authority of the ATA
    #[account(mut)]
    pub ata_authority: AccountInfo<'info>,

    /// Token Mint. The token that is bridged in.
    #[account(
        mut, 
        seeds = [SEED_PREFIX_MINT],
        bump
    )]
    pub token_mint: Account<'info, Mint>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged in.
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = ata_authority,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    // Solana SPL Token Program
    pub token_program: Program<'info, Token>,
    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,
    /// System program.
    pub system_program: Program<'info, System>,
}
