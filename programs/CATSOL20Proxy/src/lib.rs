use anchor_lang::prelude::*;

pub mod actions;
pub use actions::*;

pub mod cat_struct;
pub use cat_struct::*;

pub mod error;
pub use error::*;

pub mod state;
pub use state::*;

pub mod constants;
pub use constants::*;

pub mod utils;
pub use utils::*;

declare_id!("4dhPGRoFDPFWPVue1xReyazzd1r1k93qyn7qLsNNNeuf");

#[program]
pub mod cat_sol20_proxy {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Initialize::initialize(ctx)
    }

    pub fn transfer_ownership(ctx: Context<TransferOwnership>) -> Result<()> {
        TransferOwnership::transfer_ownership(ctx)
    }

    pub fn register_emitter( ctx: Context<RegisterEmitter>, params: RegisterEmitterParams) -> Result<()> {
        RegisterEmitter::register_emitter(ctx, &params)
    }

    pub fn bridge_out(ctx: Context<BridgeOut>, params: BridgeOutParams) -> Result<()> {
        BridgeOut::bridge_out(ctx, &params)
    }

    pub fn bridge_in(ctx: Context<BridgeIn>, vaa_hash: [u8; 32]) -> Result<()> {
        BridgeIn::bridge_in(ctx, vaa_hash)
    }
}
