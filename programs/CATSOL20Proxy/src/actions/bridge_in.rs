use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    associated_token::{self, AssociatedToken},
    token::{transfer, Transfer, Mint, Token, TokenAccount},
};

use crate::{
    constants::*,
    utils_cat::*,
    error::ErrorFactory,
    cat_struct::CATSOLStructs,
    state::{Config, ForeignEmitter, Received}
};


#[derive(Accounts)]
#[instruction(vaa_hash: [u8; 32])]
pub struct BridgeIn<'info> {
    /// Owner will initialize an account that tracks his own payloads
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Token Mint. The token that is Will be bridged out
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged out
    #[account(mut)]
    pub token_user_ata: Account<'info, TokenAccount>,

    // Token Mint ATA. Its an Associated Token Account owned by the Program that will hold the locked tokens
    #[account(
        mut,
        seeds = [SEED_PREFIX_LOCK, token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = token_mint_ata,
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
    /// Received account. [`receive_message`](crate::receive_message) will
    /// deserialize the Wormhole message's payload and save it to this account.
    /// This account cannot be overwritten, and will prevent Wormhole message
    /// replay with the same sequence.
    pub received: Account<'info, Received>,

    #[account(
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            &posted.emitter_chain().to_le_bytes()[..]
        ],
        bump,
        constraint = foreign_emitter.verify(posted.emitter_address()) @ ErrorFactory::InvalidForeignEmitter
    )]
    /// Foreign emitter account. The posted message's `emitter_address` must
    /// agree with the one we have registered for this message's `emitter_chain`
    /// (chain ID). Read-only.
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    /// System program.
    pub system_program: Program<'info, System>,
}


impl BridgeIn<'_> {
    pub fn bridge_in(ctx: Context<BridgeIn>, vaa_hash: [u8; 32]) -> Result<()> {
        let posted_message = &ctx.accounts.posted;

        if let CATSOLStructs::CrossChainPayload { payload } = posted_message.data() {
            require!(
                payload.dest_token_chain == wormhole::CHAIN_ID_SOLANA,
                ErrorFactory::InvalidDestinationChain
            );
            
            let ata_address = associated_token::get_associated_token_address(
                &Pubkey::from(payload.dest_user_address),
                &ctx.accounts.token_mint.key(),
            );

            // Check if the ATA address is the same as the one in the payload
            require_keys_eq!(
                ata_address,
                ctx.accounts.token_user_ata.key(),
                ErrorFactory::MisMatchdATAAddress
            );

            // Normalize the amount by converting it back from the standard 8 decimals to the token's decimals
            let amount_u64: u64 = payload.amount.into();
            let decimals = ctx.accounts.token_mint.decimals;
            let normalized_amount = denormalize_amount(amount_u64, decimals);

            // Mint the tokens
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = Transfer {
                from: ctx.accounts.token_mint_ata.to_account_info(),
                to: ctx.accounts.token_user_ata.to_account_info(),
                authority: ctx.accounts.token_mint_ata.to_account_info(),
            };

            let bump = *ctx
                .bumps
                .get("token_mint_ata")
                .ok_or(ErrorFactory::BumpNotFound)?;

            let cpi_signer_seeds = &[
                b"cat_sol_proxy".as_ref(),
                &ctx.accounts.token_mint.key().to_bytes(),
                &[bump],
            ];

            let cpi_signer = &[&cpi_signer_seeds[..]];

            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, cpi_signer);

            transfer(cpi_ctx, normalized_amount)?;

            //Save keccak256 hash.
            let received = &mut ctx.accounts.received;
            received.wormhole_message_hash = vaa_hash;

            // Done
            Ok(())
        } else {
            Err(ErrorFactory::InvalidMessage.into())
        }
    }
}