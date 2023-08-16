use anchor_lang::prelude::*;

pub use context::*;
pub use error::*;

pub mod config;
pub mod context;
pub mod error;

declare_id!("BxEc6d3UuHJ7cdKxHQ8NJvc9bnW8bzwfWXXg4rmteFMk");

#[program]
pub mod test_token {
    use super::*;
    use anchor_spl::token::{mint_to, MintTo};

    pub fn initialize(
        ctx: Context<Initialize>,
        _decimals: u8,
        max_supply: u64,
        amount: u64,
    ) -> Result<()> {
        
        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.owner.key();

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
                msg!("Error Minting Tokens: {:?}", e);
                return Err(e);
            }
        }

        // Set the Max and Minted Supply
        config.max_supply = max_supply;
        config.minted_supply = ctx.accounts.token_mint.supply;

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
                msg!("Error Minting Tokens: {:?}", e);
                return Err(e);
            }
        }
        // Update the Minted Supply
        config.minted_supply += amount;

        Ok(())
    }
}
