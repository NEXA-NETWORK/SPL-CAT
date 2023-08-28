use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        create_master_edition_v3, create_metadata_accounts_v3,
        sign_metadata, CreateMasterEditionV3,
        CreateMetadataAccountsV3, Metadata, SignMetadata,
    },
    token::{mint_to, Mint, MintTo, Token, TokenAccount},
};

use mpl_token_metadata::{
    pda::{find_master_edition_account, find_metadata_account},
    state::{CollectionDetails, Creator, DataV2},
};

use crate::{
    constants::*,
    error::ErrorFactory,
    state::Config
};


#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CreateCollectionParams {
    pub max_supply: u64,
    pub name: String,
    pub symbol: String,
    pub base_uri: String,
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
        seeds = [SEED_PREFIX_COLLECTION],
        bump,
        payer = owner,
        mint::decimals = 0,
        mint::authority = collection_mint,
        mint::freeze_authority = collection_mint,
    )]
    pub collection_mint: Account<'info, Mint>,

    /// Token Account. Its an Associated Token Account that will hold the
    /// tokens that are bridged out. It is owned by the program.
    /// Locked tokens will be transferred to this account
    #[account(
        init,
        payer = owner,
        associated_token::mint = collection_mint,
        associated_token::authority = owner,
    )]
    pub token_account: Account<'info, TokenAccount>,

    /// CHECK:
    #[account(
        mut,
        address=find_metadata_account(&collection_mint.key()).0
    )]
    pub metadata_account: UncheckedAccount<'info>,

    /// CHECK:
    #[account(
        mut,
        address=find_master_edition_account(&collection_mint.key()).0
    )]
    pub master_edition: UncheckedAccount<'info>,

    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}


impl CreateCollection<'_> {
    pub fn create_collection(
        ctx: Context<CreateCollection>,
        params: &CreateCollectionParams,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Set the Max, Minted Supply and Base URI
        config.max_supply = params.max_supply;
        config.minted_supply = 0;
        config.base_uri = params.base_uri.clone();

        // PDA Seed for Signing
        let bump = *ctx
            .bumps
            .get("collection_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

        let signer_seeds: &[&[&[u8]]] = &[&[
            SEED_PREFIX_COLLECTION,
            &[bump],
        ]];

        // mint collection nft
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.collection_mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.collection_mint.to_account_info(),
                },
                signer_seeds,
            ),
            1,
        )?;

        msg!("Token Minted !!!");

        // create metadata account for collection nft
        create_metadata_accounts_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMetadataAccountsV3 {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    mint: ctx.accounts.collection_mint.to_account_info(),
                    mint_authority: ctx.accounts.collection_mint.to_account_info(), // use pda mint address as mint authority
                    update_authority: ctx.accounts.collection_mint.to_account_info(), // use pda mint as update authority
                    payer: ctx.accounts.owner.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                },
                &signer_seeds,
            ),
            DataV2 {
                name: params.name.clone(),
                symbol: params.symbol.clone(),
                uri: params.base_uri.clone(),
                seller_fee_basis_points: 0,
                creators: Some(vec![Creator {
                    address: ctx.accounts.owner.key(),
                    verified: false,
                    share: 100,
                }]),
                collection: None,
                uses: None,
            },
            true,
            true,
            Some(CollectionDetails::V1 { size: params.max_supply }), // set as collection nft
        )?;

         // create master edition account for collection nft
         create_master_edition_v3(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                CreateMasterEditionV3 {
                    payer: ctx.accounts.owner.to_account_info(),
                    mint: ctx.accounts.collection_mint.to_account_info(),
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

        // verify creator on metadata account
        sign_metadata(CpiContext::new(
            ctx.accounts.metadata_program.to_account_info(),
            SignMetadata {
                creator: ctx.accounts.owner.to_account_info(),
                metadata: ctx.accounts.metadata_account.to_account_info(),
            },
        ))?;

        Ok(())
    }
}
       