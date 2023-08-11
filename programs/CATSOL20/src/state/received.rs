use anchor_lang::prelude::*;

#[account]
#[derive(Default, Debug)]
/// Received account.
pub struct Received {
    /// Keccak256 hash of verified Wormhole message.
    pub wormhole_message_hash: [u8; 32],
}



impl Received {
    pub const MAXIMUM_SIZE: usize = 8 // discriminator
        + 32 // wormhole_message_hash
    ;
    /// AKA `b"received"`.
    pub const SEED_PREFIX: &'static [u8; 8] = b"received";
}
