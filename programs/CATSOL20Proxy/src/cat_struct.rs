use anchor_lang::{prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use std::io::{self, Read, Write};

// NOTE: Solana Uses Big Endian, Ethereum uses Little Endian
// NOTE: Solana uses 8 byte u64, Ethereum uses 32 byte u256

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrossChainStruct {
    pub amount: U256,
    pub token_address: [u8; 32],
    pub token_chain: u16,
    pub to_address: [u8; 32],
    pub to_chain: u16,
    pub token_decimals: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct U256 {
    bytes: [u8; 32],
}

impl From<u64> for U256 {
    fn from(val: u64) -> Self {
        let mut bytes = [0u8; 32];
        bytes[24..].copy_from_slice(&val.to_be_bytes());
        Self { bytes }
    }
}

impl Into<u64> for U256 {
    fn into(self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.bytes[24..]);
        u64::from_be_bytes(bytes)
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
    fn deserialize(bytes: &mut &[u8]) -> io::Result<Self> {
        if bytes.len() == 32 {
            // Assume this is an Alive variant, as it's the size of a Pubkey
            let program_id = Pubkey::deserialize(bytes)?;
            Ok(CATSOLStructs::Alive { program_id })
        } else {
            // Assume this is a CrossChainPayload variant otherwise
            let mut amount_bytes = [0u8; 32];
            bytes.read_exact(&mut amount_bytes)?;
            let amount = U256 {
                bytes: amount_bytes,
            };

            let mut token_address = [0u8; 32];
            bytes.read_exact(&mut token_address)?;

            let mut token_chain_bytes = [0u8; 2];
            bytes.read_exact(&mut token_chain_bytes)?;
            let token_chain = u16::from_be_bytes(token_chain_bytes);

            let mut to_address = [0u8; 32];
            bytes.read_exact(&mut to_address)?;

            let mut to_chain_bytes = [0u8; 2];
            bytes.read_exact(&mut to_chain_bytes)?;
            let to_chain = u16::from_be_bytes(to_chain_bytes);

            let mut token_decimals_bytes = [0u8; 1];
            bytes.read_exact(&mut token_decimals_bytes)?;
            let token_decimals = u8::from_le_bytes(token_decimals_bytes);

            let payload = CrossChainStruct {
                amount,
                token_address,
                token_chain,
                to_address,
                to_chain,
                token_decimals,
            };
            Ok(CATSOLStructs::CrossChainPayload { payload })
        }
    }
}
