use anchor_lang::{AnchorDeserialize, AnchorSerialize};
use primitive_types::U256;
use std::io::{self, Read, Write};


#[derive(PartialEq, Debug, Clone)]
pub struct CrossChainPayload {
    pub amount: u64,
    pub token_address: [u8; 32],
    pub token_chain: u16,
    pub to_address: [u8; 32],
    pub to_chain: u16,
}

#[derive(PartialEq, Debug, Clone)]
pub struct SignatureVerification {
    pub custodian: [u8; 20],
    pub valid_till: U256,
    pub signature: Vec<u8>,
}

#[derive(PartialEq, Debug, Clone)]
pub enum CATSOLStructs {
    CrossChainPayload { payload: CrossChainPayload },
    SignatureVerification { verification: SignatureVerification },
}

impl AnchorSerialize for CATSOLStructs {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            CATSOLStructs::CrossChainPayload { payload } => {
                let mut payload_bytes = Vec::new();
                payload_bytes.extend_from_slice(&payload.amount.to_be_bytes());
                payload_bytes.extend_from_slice(&payload.token_address);
                payload_bytes.extend_from_slice(&payload.to_address);
                payload_bytes.extend_from_slice(&payload.token_chain.to_be_bytes());
                payload_bytes.extend_from_slice(&payload.to_chain.to_be_bytes());
                writer.write_all(&payload_bytes)?;
                Ok(())
            }
            CATSOLStructs::SignatureVerification { verification } => {
                let mut verification_bytes = Vec::new();
                let mut valid_till = [0u8; 32];
                verification.valid_till.to_big_endian(&mut valid_till);
                verification_bytes.extend_from_slice(&verification.custodian);
                verification_bytes.extend_from_slice(&valid_till);
                verification_bytes.extend_from_slice(&verification.signature);
                writer.write_all(&verification_bytes)?;
                Ok(())
            }
        }
    }
}

impl AnchorDeserialize for CATSOLStructs {

    fn deserialize(bytes: &mut &[u8]) -> io::Result<Self> {
        let mut amount_bytes = [0u8; 8];
        bytes.read_exact(&mut amount_bytes)?;
        let amount = u64::from_be_bytes(amount_bytes);

        let mut token_address = [0u8; 32];
        bytes.read_exact(&mut token_address)?;

        let mut to_address = [0u8; 32];
        bytes.read_exact(&mut to_address)?;[0u8; 2];

        let mut token_chain_bytes = [0u8; 2];
        bytes.read_exact(&mut token_chain_bytes)?;
        let token_chain = u16::from_be_bytes(token_chain_bytes);

        let mut to_chain_bytes = [0u8; 2];
        bytes.read_exact(&mut to_chain_bytes)?;
        let to_chain = u16::from_be_bytes(to_chain_bytes);

        let payload = CrossChainPayload {
            amount,
            token_address,
            token_chain,
            to_address,
            to_chain,
        };
        Ok(CATSOLStructs::CrossChainPayload { payload })
    }
}
