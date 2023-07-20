use anchor_lang::prelude::error_code;

#[error_code]
/// Errors relevant to this program's malfunction.
pub enum ErrorFactory {
    #[msg("InvalidWormholeConfig")]
    /// Specified Wormhole bridge data PDA is wrong.
    InvalidWormholeConfig,

    #[msg("InvalidWormholeFeeCollector")]
    /// Specified Wormhole fee collector PDA is wrong.
    InvalidWormholeFeeCollector,

    #[msg("InvalidWormholeEmitter")]
    /// Specified program's emitter PDA is wrong.
    InvalidWormholeEmitter,

    #[msg("InvalidWormholeSequence")]
    /// Specified emitter's sequence PDA is wrong.
    InvalidWormholeSequence,

    #[msg("InvalidSysvar")]
    /// Specified sysvar is wrong.
    InvalidSysvar,

    #[msg("OwnerOnly")]
    /// Only the program's owner is permitted.
    OwnerOnly,

    #[msg("InvalidForeignEmitter: Invalid Chain ID or Zero Address (Solana Chain ID is not allowed)")]
    /// Specified foreign emitter has a bad chain ID or zero address.
    InvalidForeignEmitter,

    #[msg("BumpNotFound")]
    /// Bump not found in `bumps` map.
    BumpNotFound,

    #[msg("InvalidMessage")]
    /// Deserialized message has unexpected payload type.
    InvalidMessage,

    #[msg("The Off Chain ATA Address Does Not Match The Address of the Payload")]
    /// The ATA sent in the instruction does not match the ATA of the payload from wormhole.
    InvalidATAAddress,

    #[msg("InvalidAmount: The difference between local and foreign decimals is too large and is causing an overflow.")]
    /// The difference between local and foreign decimals is too large and is causing an overflow.
    InvalidAmount,

    #[msg("InvalidAmount: The amount is exceeding the maximum amount allowed to be minted.")]
    /// The amount is too large and is exceeding the maximum amount allowed to be minted.
    IvalidMintAmount,

    #[msg("MintToFailed: The mint to instruction failed.")]
    MintToFailed,

    #[msg("TokenBurnFailed: The token burn instruction failed.")]
    TokenBurnFailed,
}
