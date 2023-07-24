use anchor_lang::prelude::*;

#[account]
#[derive(Default)]
/// Config account data.
pub struct Config {
    /// Program's owner.
    pub owner: Pubkey,
    /// Minted supply.
    pub minted_supply: u64,
    /// Max supply.
    pub max_supply: u64,
}

impl Config {
    pub const MAXIMUM_SIZE: usize = 8 // discriminator
        + 32 // owner
        + 8 // minted_supply
        + 8 // max_supply   
    ;
    /// AKA `b"config"`.
    pub const SEED_PREFIX: &'static [u8; 6] = b"config";
}
