use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_master_edition_v3, create_metadata_accounts_v3,
        set_and_verify_sized_collection_item, CreateMasterEditionV3,
        CreateMetadataAccountsV3, Metadata, SetAndVerifySizedCollectionItem,
    },
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};
use mpl_token_metadata::{
    pda::{find_master_edition_account, find_metadata_account},
    state::DataV2,
};

use crate::{constants::*, error::ErrorFactory, state::Config};

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct MintParams {
    pub name: String,
    pub symbol: String,
    pub mint_uri: String,
}

#[derive(Accounts)]
#[instruction(params: MintParams)]
pub struct MintNFT<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        has_one = owner @ ErrorFactory::OwnerOnly,
        seeds = [Config::SEED_PREFIX],
        bump
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut,
        seeds = [SEED_PREFIX_COLLECTION],
        bump,
    )]
    pub collection_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(
        mut,
        address=find_metadata_account(&collection_mint.key()).0
    )]
    pub collection_metadata_account: UncheckedAccount<'info>,

    /// CHECK:
    #[account(
        mut,
        address=find_master_edition_account(&collection_mint.key()).0
    )]
    pub collection_master_edition: UncheckedAccount<'info>,

    #[account(
        init,
        payer = owner,
        mint::decimals = 0,
        mint::authority = collection_mint,
        mint::freeze_authority = collection_mint
    )]
    pub nft_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(
        mut,
        address=find_metadata_account(&nft_mint.key()).0
    )]
    pub metadata_account: UncheckedAccount<'info>,

    /// CHECK:
    #[account(
        mut,
        address=find_master_edition_account(&nft_mint.key()).0
    )]
    pub master_edition: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

impl MintNFT<'_> {
    pub fn mint_nft(ctx: Context<MintNFT>, params: &MintParams) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Check if the amount doesn't exceed the max supply
        if config.minted_supply + 1 >= config.max_supply {
            return Err(ErrorFactory::InvalidMintAmount.into());
        }

        // PDA Seed for Signing
        let bump = *ctx
            .bumps
            .get("collection_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

        let signer_seeds: &[&[&[u8]]] = &[&[SEED_PREFIX_COLLECTION, &[bump]]];

        // mint nft in collection
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.collection_mint.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        // create metadata account for nft in collection
        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    mint_authority: ctx.accounts.collection_mint.to_account_info(),
                    update_authority: ctx.accounts.collection_mint.to_account_info(),
                    payer: ctx.accounts.owner.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &signer_seeds,
            ),
            DataV2 {
                name: params.name.clone(),
                symbol: params.symbol.clone(),
                uri: params.mint_uri.clone(),
                seller_fee_basis_points: 0,
                creators: None,
                collection: None,
                uses: None,
            },
            true,
            true,
            None,
        )?;

        // create master edition account for nft in collection
        create_master_edition_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMasterEditionV3 {
                    payer: ctx.accounts.owner.to_account_info(),
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    edition: ctx.accounts.master_edition.to_account_info(),
                    mint_authority: ctx.accounts.collection_mint.to_account_info(),
                    update_authority: ctx.accounts.collection_mint.to_account_info(),
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &signer_seeds,
            ),
            Some(0),
        )?;

        // verify nft as part of collection
        set_and_verify_sized_collection_item(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                SetAndVerifySizedCollectionItem {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    collection_authority: ctx.accounts.collection_mint.to_account_info(),
                    payer: ctx.accounts.owner.to_account_info(),
                    update_authority: ctx.accounts.collection_mint.to_account_info(),
                    collection_mint: ctx.accounts.collection_mint.to_account_info(),
                    collection_metadata: ctx.accounts.collection_metadata_account.to_account_info(),
                    collection_master_edition: ctx
                        .accounts
                        .collection_master_edition
                        .to_account_info(),
                },
                &signer_seeds,
            ),
            None,
        )?;

        Ok(())
    }
}
