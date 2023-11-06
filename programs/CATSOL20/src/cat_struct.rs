use anchor_lang::prelude::*;
use anchor_lang::{prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use std::io::{self, Read, Write};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrossChainStruct {
    pub amount: U256,
    pub token_decimals: u8,
    pub source_token_address: [u8; 32],
    pub source_user_address: [u8; 32],
    pub source_token_chain: U256,
    pub dest_token_address: [u8; 32],
    pub dest_user_address: [u8; 32],
    pub dest_token_chain: U256,
}

#[derive(Default, AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct U256 {
    pub bytes: [u8; 32],
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
            CATSOLStructs::CrossChainPayload { payload } => {
                payload.amount.serialize(writer)?;
                writer.write_all(&payload.token_decimals.to_le_bytes())?;
                writer.write_all(&payload.source_token_address)?;
                writer.write_all(&payload.source_user_address)?;
                payload.source_token_chain.serialize(writer)?;
                writer.write_all(&payload.dest_token_address)?;
                writer.write_all(&payload.dest_user_address)?;
                payload.dest_token_chain.serialize(writer)?;
                Ok(())
            }
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

            let mut token_decimals_bytes = [0u8; 1];
            bytes.read_exact(&mut token_decimals_bytes)?;
            let token_decimals = u8::from_le_bytes(token_decimals_bytes);

            let mut source_token_address = [0u8; 32];
            bytes.read_exact(&mut source_token_address)?;

            let mut source_user_address = [0u8; 32];
            bytes.read_exact(&mut source_user_address)?;

            let mut source_token_chain_bytes = [0u8; 32];
            bytes.read_exact(&mut source_token_chain_bytes)?;
            let source_token_chain = U256 {
                bytes: source_token_chain_bytes,
            };

            let mut dest_token_address = [0u8; 32];
            bytes.read_exact(&mut dest_token_address)?;

            let mut dest_user_address = [0u8; 32];
            bytes.read_exact(&mut dest_user_address)?;

            let mut dest_token_chain_bytes = [0u8; 32];
            bytes.read_exact(&mut dest_token_chain_bytes)?;
            let dest_token_chain = U256 {
                bytes: dest_token_chain_bytes,
            };

            let payload = CrossChainStruct {
                amount,
                token_decimals,
                source_token_address,
                source_user_address,
                source_token_chain,
                dest_token_address,
                dest_user_address,
                dest_token_chain,
            };
            Ok(CATSOLStructs::CrossChainPayload { payload })
        }
    }
    
    fn deserialize_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        Self::deserialize(&mut &buf[..])
    }
}
