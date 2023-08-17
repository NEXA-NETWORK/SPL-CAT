use anchor_lang::prelude::*;
use wormhole_anchor_sdk::wormhole;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{burn, Burn, Mint,Token, TokenAccount},

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
    pub owner: Signer<'info>,

    /// CHECK: This is the authority of the ATA
    #[account(mut)]
    pub user: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [Config::SEED_PREFIX, user.key().as_ref()],
        bump,
    )]
    pub config: Box<Account<'info, Config>>,

    #[account(
        mut, 
        seeds = [SEED_PREFIX_MINT, user.key().as_ref()],
        bump
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = user,
    )]
    pub token_user_ata: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub wormhole_program: Program<'info, wormhole::program::Wormhole>,

    #[account(
        mut,
        address = config.wormhole.bridge @ ErrorFactory::InvalidWormholeConfig,
    )]
    pub wormhole_bridge: Account<'info, wormhole::BridgeData>,

    #[account(
        mut,
        address = config.wormhole.fee_collector @ ErrorFactory::InvalidWormholeFeeCollector
    )]
    pub wormhole_fee_collector: Account<'info, wormhole::FeeCollector>,

    #[account(
        seeds = [WormholeEmitter::SEED_PREFIX, token_mint.key().as_ref()],
        bump,
    )]
    pub wormhole_emitter: Account<'info, WormholeEmitter>,

    #[account(
        mut,
        address = config.wormhole.sequence @ ErrorFactory::InvalidWormholeSequence
    )]
    pub wormhole_sequence: Account<'info, wormhole::SequenceTracker>,

    #[account(
        mut,
        seeds = [
            SEED_PREFIX_SENT,
            wormhole_emitter.key().as_ref(),
            &wormhole_sequence.next_value().to_le_bytes()[..]
        ],
        bump,
    )]
    /// CHECK: Wormhole Message. [`wormhole::post_message`] requires this account be mutable.
    pub wormhole_message: UncheckedAccount<'info>,

    #[account(
        seeds = [
            ForeignEmitter::SEED_PREFIX,
            config.key().as_ref(),
            &params.recipient_chain.to_le_bytes()[..]
        ],
        bump,
        constraint = foreign_emitter.chain == params.recipient_chain
    )]
    pub foreign_emitter: Account<'info, ForeignEmitter>,

    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
    pub rent: Sysvar<'info, Rent>,
}

impl BridgeOut<'_> {
    pub fn bridge_out(ctx: Context<BridgeOut>, params: BridgeOutParams ) -> Result<()> {
        // // Pay the Fee
        // let fee = ctx.accounts.wormhole_bridge.fee();
        // if fee > 0 {
        //     solana_program::program::invoke(
        //         &solana_program::system_instruction::transfer(
        //             &ctx.accounts.owner.key(),
        //             &ctx.accounts.wormhole_fee_collector.key(),
        //             fee,
        //         ),
        //         &ctx.accounts.to_account_infos(),
        //     )?;
        // }

        // // Burn the tokens
        // let cpi_program = ctx.accounts.token_program.to_account_info();
        // let cpi_accounts = Burn {
        //     mint: ctx.accounts.token_mint.to_account_info(),
        //     from: ctx.accounts.token_user_ata.to_account_info(),
        //     authority: ctx.accounts.owner.to_account_info(),
        // };
        // let bump = *ctx
        //     .bumps
        //     .get("token_mint")
        //     .ok_or(ErrorFactory::BumpNotFound)?;

        // let cpi_signer_seeds = &[
        //     b"spl_cat_token".as_ref(),
        //     &[bump],
        // ];
        // let cpi_signer = &[&cpi_signer_seeds[..]];
        
        // let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, cpi_signer);

        // burn(cpi_ctx, params.amount)?;

        // // Normalize the amount to a Standard 8 decimals
        // let decimals = ctx.accounts.token_mint.decimals;
        // let foreign_amount = normalize_amount(params.amount, decimals);

        // // Create the payload
        // let payload = CrossChainStruct {
        //     amount: U256::from(foreign_amount),
        //     token_address: ctx.accounts.token_user_ata.key().to_bytes(),
        //     token_chain: wormhole::CHAIN_ID_SOLANA,
        //     to_address: params.recipient,
        //     to_chain: params.recipient_chain,
        //     token_decimals: ctx.accounts.token_mint.decimals,
        // };

        // // Serialize the payload
        // let cat_sol_struct = CATSOLStructs::CrossChainPayload { payload };
        // let mut encoded_payload: Vec<u8> = Vec::new();
        // cat_sol_struct.serialize(&mut encoded_payload)?;

        // let wormhole_emitter = &ctx.accounts.wormhole_emitter;
        // let config = &ctx.accounts.config;

        // wormhole::post_message(
        //     CpiContext::new_with_signer(
        //         ctx.accounts.wormhole_program.to_account_info(),
        //         wormhole::PostMessage {
        //             config: ctx.accounts.wormhole_bridge.to_account_info(),
        //             message: ctx.accounts.wormhole_message.to_account_info(),
        //             emitter: wormhole_emitter.to_account_info(),
        //             sequence: ctx.accounts.wormhole_sequence.to_account_info(),
        //             payer: ctx.accounts.owner.to_account_info(),
        //             fee_collector: ctx.accounts.wormhole_fee_collector.to_account_info(),
        //             clock: ctx.accounts.clock.to_account_info(),
        //             rent: ctx.accounts.rent.to_account_info(),
        //             system_program: ctx.accounts.system_program.to_account_info(),
        //         },
        //         &[
        //             &[
        //                 SEED_PREFIX_SENT,
        //                 &ctx.accounts.wormhole_sequence.next_value().to_le_bytes()[..],
        //                 &[*ctx
        //                     .bumps
        //                     .get("wormhole_message")
        //                     .ok_or(ErrorFactory::BumpNotFound)?],
        //             ],
        //             &[wormhole::SEED_PREFIX_EMITTER, &[wormhole_emitter.bump]],
        //         ],
        //     ),
        //     config.batch_id,
        //     encoded_payload,
        //     config.finality.into(),
        // )?;

        // Done.
        Ok(())
    }
}
