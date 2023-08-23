use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    associated_token::{self, AssociatedToken},
    token::{mint_to, MintTo, Mint, Token, TokenAccount},
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

    /// CHECK: ATA Authority. The authority of the ATA that will hold the bridged tokens.
    #[account(mut)]
    pub ata_authority: UncheckedAccount<'info>,

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
                payload.to_chain == wormhole::CHAIN_ID_SOLANA,
                ErrorFactory::InvalidDestinationChain
            );
            
            let ata_address = associated_token::get_associated_token_address(
                &Pubkey::from(payload.to_address),
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
            let cpi_accounts = MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.token_user_ata.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            };
            let bump = *ctx
                .bumps
                .get("token_mint")
                .ok_or(ErrorFactory::BumpNotFound)?;

            let cpi_signer_seeds = &[
                b"spl_cat_token".as_ref(),
                &[bump],
            ];
            let cpi_signer = &[&cpi_signer_seeds[..]];

            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, cpi_signer);

            mint_to(cpi_ctx, normalized_amount)?;

            //Save batch ID, keccak256 hash and message payload.
            let received = &mut ctx.accounts.received;
            received.wormhole_message_hash = vaa_hash;

            // Done
            Ok(())
        } else {
            Err(ErrorFactory::InvalidMessage.into())
        }
    }
}