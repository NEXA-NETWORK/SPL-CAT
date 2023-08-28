use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{burn_nft, BurnNft, Metadata},
    token::{Mint, Token, TokenAccount},
};
use wormhole_anchor_sdk::wormhole;

use crate::{
    cat_struct::{CATSOLStructs, CrossChainStruct, U256},
    constants::*,
    error::ErrorFactory,
    state::{Config, ForeignEmitter, WormholeEmitter},
};

use mpl_token_metadata::pda::{find_master_edition_account, find_metadata_account};

#[derive(Clone, AnchorDeserialize, AnchorSerialize)]
pub struct BridgeOutParams {
    pub token_id: u64,
    pub recipient_chain: u16,
    pub recipient: [u8; 32],
}
#[derive(Accounts)]
#[instruction(params: BridgeOutParams)]
pub struct BridgeOut<'info> {
    #[account(mut)]
    /// Owner will pay Wormhole fee to post a message and pay for the associated token account.
    pub owner: Signer<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [SEED_PREFIX_COLLECTION],
        bump,
    )]
    pub collection_mint: Account<'info, Mint>,

    /// CHECK:
    #[account(
        address=find_metadata_account(&collection_mint.key()).0
    )]
    pub collection_metadata_account: UncheckedAccount<'info>,


    #[account(
        mut,
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
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    pub token_account: Account<'info, TokenAccount>,

    // Solana SPL Token Program
    pub token_program: Program<'info, Token>,

    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,

    pub metadata_program: Program<'info, Metadata>,

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

    #[account(
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            &params.recipient_chain.to_le_bytes()[..]
        ],
        bump,
        constraint = foreign_emitter.chain == params.recipient_chain
    )]
    /// Foreign Emitter account should exist for the recipient chain. Read-only.
    /// We're just checking if the account exists and is initialized.
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    /// System program.
    pub system_program: Program<'info, System>,

    /// Clock sysvar.
    pub clock: Sysvar<'info, Clock>,

    /// Rent sysvar.
    pub rent: Sysvar<'info, Rent>,
}

impl BridgeOut<'_> {
    pub fn bridge_out(ctx: Context<BridgeOut>, params: BridgeOutParams) -> Result<()> {
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


        // Need a check here to see if the passed Metadata account has the same URI as the one in the collection
        // Require Check
        // Get Metadata account URI and check if the token_id is same.



        // PDA Seed for Signing
        let bump = *ctx
            .bumps
            .get("collection_mint")
            .ok_or(ErrorFactory::BumpNotFound)?;

        let signer_seeds: &[&[&[u8]]] = &[&[SEED_PREFIX_COLLECTION, &[bump]]];

        // burn nft in collection
        burn_nft(
            CpiContext::new_with_signer(
                ctx.accounts.metadata_program.to_account_info(),
                BurnNft {
                    metadata: ctx.accounts.metadata_account.to_account_info(),
                    owner: ctx.accounts.user.to_account_info(),
                    mint: ctx.accounts.nft_mint.to_account_info(),
                    token: ctx.accounts.token_account.to_account_info(),
                    edition: ctx.accounts.master_edition.to_account_info(),
                    spl_token: ctx.accounts.token_program.to_account_info(),
                },
                signer_seeds,
            ),
            Some(ctx.accounts.collection_metadata_account.key()),
        )?;

        let nft_uri: String = ctx.accounts.config.base_uri.clone() + &params.token_id.to_string();

        // Create the payload
        let payload = CrossChainStruct {
            token_address: ctx.accounts.user.key().to_bytes(),
            token_chain: wormhole::CHAIN_ID_SOLANA,
            token_id: U256::from(params.token_id),
            uri: nft_uri,
            to_address: params.recipient,
            to_chain: params.recipient_chain,
        };

        // Serialize the payload
        let cat_sol_struct = CATSOLStructs::CrossChainPayload { payload };
        let mut encoded_payload: Vec<u8> = Vec::new();
        cat_sol_struct.serialize(&mut encoded_payload)?;

        let wormhole_emitter = &ctx.accounts.wormhole_emitter;
        let config = &ctx.accounts.config;

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
                        &ctx.accounts.wormhole_sequence.next_value().to_le_bytes()[..],
                        &[*ctx
                            .bumps
                            .get("wormhole_message")
                            .ok_or(ErrorFactory::BumpNotFound)?],
                    ],
                    &[wormhole::SEED_PREFIX_EMITTER, &[wormhole_emitter.bump]],
                ],
            ),
            config.batch_id,
            encoded_payload,
            config.finality.into(),
        )?;

        // Done.
        Ok(())
    }
}
