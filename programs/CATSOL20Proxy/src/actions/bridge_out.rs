use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Transfer, Mint,Token, TokenAccount},

};
use crate::{
    constants::*,
    utils_cat::*,
    error::ErrorFactory,
    cat_struct::{CATSOLStructs, CrossChainStruct, U256},
    state::{Config, ForeignEmitter, WormholeEmitter}
};

#[derive(Clone, AnchorDeserialize, AnchorSerialize)]
pub struct BridgeOutParams {
    pub amount: u64,
    pub recipient_chain: u16,
    pub recipient: [u8; 32],
}
#[derive(Accounts)]
#[instruction(params: BridgeOutParams)]
pub struct BridgeOut<'info> {
    #[account(mut)]
    /// Owner will pay Wormhole fee to post a message and pay for the associated token account.
    pub owner: Signer<'info>,

    /// Token Mint. The token that is Will be bridged out
    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    // Token Account. Its an Associated Token Account that will hold the
    // tokens that are bridged out
    #[account(mut)]
    pub token_user_ata: Box<Account<'info, TokenAccount>>,

    /// CHECK: Token ATA PDA. The PDA of the ATA that will hold the locked tokens. It will act
    /// as the authority as well.
    #[account(
        mut,
        seeds = [SEED_PREFIX_MINT, token_user_ata.key().as_ref()],
        bump,
      )]
    pub token_ata_pda: AccountInfo<'info>,

    // Token Mint ATA. Its an Associated Token Account owned by the Program that will hold the locked tokens
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = token_ata_pda,
    )]
    pub token_mint_ata: Account<'info, TokenAccount>,

    // Solana SPL Token Program
    pub token_program: Program<'info, Token>,
    // Associated Token Program
    pub associated_token_program: Program<'info, AssociatedToken>,

    // --------------------- Wormhole ---------------------

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
    pub fn bridge_out(ctx: Context<BridgeOut>, params: &BridgeOutParams ) -> Result<()> {
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

        // Transfer the tokens
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_accounts = Transfer {
            from: ctx.accounts.token_user_ata.to_account_info(),
            to: ctx.accounts.token_mint_ata.to_account_info(),
            authority: ctx.accounts.token_ata_pda.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        match transfer(cpi_ctx, params.amount) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }

        // Normalize the amount to a Standard 8 decimals
        let decimals = ctx.accounts.token_mint.decimals;
        let foreign_amount = normalize_amount(params.amount, decimals);

        // Create the payload
        let payload = CrossChainStruct {
            amount: U256::from(foreign_amount),
            token_address: ctx.accounts.token_user_ata.key().to_bytes(),
            token_chain: wormhole::CHAIN_ID_SOLANA,
            to_address: params.recipient,
            to_chain: params.recipient_chain,
            token_decimals: ctx.accounts.token_mint.decimals,
        };

        // Serialize the payload
        let cat_sol_struct = CATSOLStructs::CrossChainPayload { payload };
        let mut encoded_payload: Vec<u8> = Vec::new();
        cat_sol_struct.serialize(&mut encoded_payload)?;

        let wormhole_emitter = &ctx.accounts.wormhole_emitter;
        let config = &ctx.accounts.config;

        match wormhole::post_message(
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
        ) {
            Ok(_) => {}
            Err(e) => {
                return Err(e);
            }
        }

        // Done.
        Ok(())
    }
}
