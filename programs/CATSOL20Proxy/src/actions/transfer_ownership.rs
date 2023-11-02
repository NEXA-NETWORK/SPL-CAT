use anchor_lang::prelude::*;
use crate::{
    error::ErrorFactory,
    state::Config
};


#[derive(Accounts)]
pub struct TransferOwnership<'info> {
    /// The Current Owner of the Config Account
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: The new owner of the Config Account
    pub new_owner: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = owner @ ErrorFactory::OwnerOnly,
        constraint = config.owner != new_owner.key() @ ErrorFactory::AlreadyOwner,
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