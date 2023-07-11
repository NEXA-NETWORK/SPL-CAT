use anchor_lang::{prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use std::io::{self, Write};

// NOTE: Solana Uses Big Endian, Ethereum uses Little Endian
// NOTE: Solana uses 8 byte u64, Ethereum uses 32 byte u256

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CrossChainStruct {
    pub amount: U256,
    pub token_address: [u8; 32],
    pub token_chain: u16,
    pub to_address: [u8; 32],
    pub to_chain: u16,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct U256 {
    bytes: [u8; 32],
}

impl From<u64> for U256 {
    fn from(val: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[24..].copy_from_slice(&val.to_le_bytes());
        Self { bytes }
    }
}

impl Into<u64> for U256 {
    fn into(self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.bytes[24..]);
        u64::from_le_bytes(bytes)
    }
}

#[derive(Clone)]
pub enum CATSOLStructs {
    Alive { program_id: Pubkey },
    CrossChainPayload { payload: CrossChainStruct },
}

impl AnchorSerialize for CATSOLStructs {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            CATSOLStructs::Alive { program_id } => program_id.serialize(writer),
            CATSOLStructs::CrossChainPayload { payload } => payload.serialize(writer),
        }
    }
}

// Should have a Discriminator field, but we don't need it in this case as Alive is only sent once.
impl AnchorDeserialize for CATSOLStructs {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        if buf.len() == 32 {
            // Assume this is an Alive variant, as it's the size of a Pubkey
            let program_id = Pubkey::deserialize(buf)?;
            Ok(CATSOLStructs::Alive { program_id })
        } else {
            // Assume this is a CrossChainPayload variant otherwise
            let payload = CrossChainStruct::deserialize(buf)?;
            Ok(CATSOLStructs::CrossChainPayload { payload })
        }
    }
}
