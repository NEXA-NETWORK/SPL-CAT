use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Mint, MintTo, Token, TokenAccount, mint_to},
    metadata::Metadata,
};

use crate::{
    constants::*,
    error::ErrorFactory,
    state::{Config}
};

use anchor_lang::solana_program::{self, program::invoke_signed};
use mpl_token_metadata::instruction::{create_metadata_accounts_v3, create_master_edition_v3};
use mpl_token_metadata::state::Creator;

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CreatorStruct {
    pub address: Pubkey,
    pub share: u8,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CreateCollectionParams {
    pub creator_key: Pubkey,
    pub max_supply: u64,
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

#[derive(Accounts)]
#[instruction(params: CreateCollectionParams)]
/// Context used to initialize program data (i.e. config).
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner @ ErrorFactory::OwnerOnly,
        seeds = [Config::SEED_PREFIX],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        init, 
        seeds = [SEED_PREFIX_MINT],
        bump,
        payer = owner,
        mint::decimals = 0,
        mint::authority = token_mint.key(),
        mint::freeze_authority = token_mint.key(),
    )]
    pub token_mint: Account<'info, Mint>,

    /// Token Account. Its an Associated Token Account that will hold the
    /// tokens that are bridged out. It is owned by the program.
    /// Locked tokens will be transferred to this account
    #[account(
        init,
        seeds = [SEED_PREFIX_COLLECTION, token_mint.key().as_ref()],
        bump,
        payer = owner,
        token::mint = token_mint,
        token::authority = token_account_collection,
    )]
    pub token_account_collection: Account<'info, TokenAccount>,

    ///CHECK: Metadata Account
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

    ///CHECK: Master Edition 
    #[account(
        mut, 
        seeds = [
            b"metadata",
            mpl_token_metadata::id().as_ref(),
            token_mint.key().as_ref(),
            b"edition",
        ],
        bump,
        seeds::program = mpl_token_metadata::id(),
    )]
    pub master_edition: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
  
    pub clock: Sysvar<'info, Clock>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}


impl CreateCollection<'_> {
    pub fn create_collection(
        ctx: Context<CreateCollection>,
        params: &CreateCollectionParams,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Set the Max and Minted Supply
        config.max_supply = params.max_supply;
        config.minted_supply = 0;

        // Time to create a Collection NFT
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = MintTo {
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.token_account_collection.to_account_info(),
            authority: ctx.accounts.token_mint.to_account_info(),
        };

        let bump = *ctx
            .bumps
            .get("token_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

        let cpi_signer_seeds = &[
            b"spl_cat_nft".as_ref(),
            &[bump],
        ];
        let cpi_signer = &[&cpi_signer_seeds[..]];

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, cpi_signer);

        mint_to(cpi_ctx, 1)?;

        msg!("Token Minted !!!");

        let bump = *ctx
            .bumps
            .get("token_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

            let signer_seeds = &[
                b"spl_cat_nft".as_ref(),
                &[bump],
            ];

        // Create Metadata for the token.
        
            
            let creator = vec![
                Creator {
                    address: params.creator_key,
                    verified: false,
                    share: 100,
                },
            ];

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
                Some(creator),
                0,
                true,
                true,
                None,
                None,
                None,
            );

            invoke_signed(
                &create_metadata_account_ix,
                &[
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.metadata_account.to_account_info(),
                    ctx.accounts.token_mint.to_account_info(),
                    ctx.accounts.metadata_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[signer_seeds],
            )?;
        

        // Create Master Edition
        

            let master_edition_infos = vec![
            ctx.accounts.master_edition.to_account_info(),
            ctx.accounts.token_mint.to_account_info(),
            ctx.accounts.token_mint.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.metadata_account.to_account_info(),
            ctx.accounts.metadata_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
        ];
        msg!("Master Edition Account Infos Assigned");
        invoke_signed(
            &create_master_edition_v3(
                ctx.accounts.metadata_program.key(),
                ctx.accounts.master_edition.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.token_mint.key(),
                ctx.accounts.metadata_account.key(),
                ctx.accounts.owner.key(),
                Some(0),
            ),
            master_edition_infos.as_slice(),
            &[&[b"spl_cat_nft".as_ref(), &[bump]]],
        )?;

        


        Ok(())
    }
}
       