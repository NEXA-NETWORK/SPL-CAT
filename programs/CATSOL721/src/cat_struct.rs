use anchor_lang::{prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use std::io::{self, Read, Write};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrossChainStruct {
    pub token_address: [u8; 32],
    pub token_chain: u16,
    pub token_id: U256,
    pub uri: String,
    pub to_address: [u8; 32],
    pub to_chain: u16,
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
            CATSOLStructs::CrossChainPayload { payload } => {
                writer.write_all(&payload.token_address)?;
                writer.write_all(&payload.token_chain.to_be_bytes())?;
                payload.token_id.serialize(writer)?;
                writer.write_all(&payload.uri.as_bytes())?;
                writer.write_all(&payload.to_address)?;
                writer.write_all(&payload.to_chain.to_be_bytes())?;
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
            let mut token_address = [0u8; 32];
            bytes.read_exact(&mut token_address)?;

            let mut token_chain_bytes = [0u8; 2];
            bytes.read_exact(&mut token_chain_bytes)?;
            let token_chain = u16::from_be_bytes(token_chain_bytes);

            let mut token_id_bytes = [0u8; 32];
            bytes.read_exact(&mut token_id_bytes)?;
            let token_id = U256 {
                bytes: token_id_bytes,
            };

            let mut uri_bytes = [0u8; 32];
            bytes.read_exact(&mut uri_bytes)?;
            let uri = String::from_utf8(uri_bytes.to_vec()).unwrap();
            
            let mut to_address = [0u8; 32];
            bytes.read_exact(&mut to_address)?;

            let mut to_chain_bytes = [0u8; 2];
            bytes.read_exact(&mut to_chain_bytes)?;
            let to_chain = u16::from_be_bytes(to_chain_bytes);

            let payload = CrossChainStruct {
                token_address,
                token_chain,
                token_id,
                uri,
                to_address,
                to_chain,
            };
            Ok(CATSOLStructs::CrossChainPayload { payload })
        }
    }
}
