use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};
use anchor_spl::token::{mint_to,  MintTo};
use crate::{
    constants::*,
    error::ErrorFactory,
    state::Config,
};

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        has_one = owner @ ErrorFactory::OwnerOnly,
        seeds = [Config::SEED_PREFIX],
        bump
    )]
    pub config: Box<Account<'info, Config>>,

    /// CHECK: This is the authority of the ATA
    #[account(mut)]
    pub ata_authority: AccountInfo<'info>,

    #[account(
        mut, 
        seeds = [SEED_PREFIX_MINT],
        bump
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = ata_authority,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}


impl MintTokens<'_> {

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
    
}