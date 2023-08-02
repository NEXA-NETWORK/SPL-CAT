use anchor_lang::prelude::*;
use crate::{
    error::ErrorFactory,
    state::Config
};


#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub new_owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner @ ErrorFactory::OwnerOnly,
        seeds = [Config::SEED_PREFIX],
        bump
    )]
    pub config: Box<Account<'info, Config>>,
}

impl TransferOwnership<'_> {
    pub fn transfer_ownership(ctx: Context<TransferOwnership>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.new_owner.key();
        Ok(())
    }
}