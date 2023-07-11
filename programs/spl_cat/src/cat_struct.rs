use anchor_lang::{prelude::Pubkey, AnchorDeserialize, AnchorSerialize};
use primitive_types::U256;
use std::io::{self, Read, Write};

#[derive(Clone, Debug)]
pub struct CrossChainStruct {
    pub amount: U256,
    pub token_address: [u8; 32],
    pub token_chain: u16,
    pub to_address: [u8; 32],
    pub to_chain: u16,
}

impl AnchorSerialize for CrossChainStruct {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // amount
        let mut amount = [0u8; 32];
        self.amount.to_little_endian(&mut amount);
        writer.write_all(&amount)?;
        // token_address
        writer.write_all(&self.token_address)?;
        // token_chain
        self.token_chain.serialize(writer)?;
        // to_address
        writer.write_all(&self.to_address)?;
        // to_chain
        self.to_chain.serialize(writer)?;
        Ok(())
    }
}

impl AnchorDeserialize for CrossChainStruct {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        // amount
        let mut amount = [0u8; 32];
        buf.read_exact(&mut amount)?;
        let amount = U256::from_little_endian(&amount);
        // token_address
        let mut token_address = [0u8; 32];
        buf.read_exact(&mut token_address)?;
        // token_chain
        let token_chain = u16::deserialize(buf)?;
        // to_address
        let mut to_address = [0u8; 32];
        buf.read_exact(&mut to_address)?;
        // to_chain
        let to_chain = u16::deserialize(buf)?;
        Ok(CrossChainStruct {
            amount,
            token_address,
            token_chain,
            to_address,
            to_chain,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SignatureVerification {
    pub custodian: [u8; 20],
    pub valid_till: U256,
    pub signature: Vec<u8>,
}

impl AnchorSerialize for SignatureVerification {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // custodian
        writer.write_all(&self.custodian)?;
        // valid_till
        let mut valid_till = [0u8; 32];
        self.valid_till.to_little_endian(&mut valid_till);
        writer.write_all(&valid_till)?;
        // signature
        writer.write_all(&self.signature)?;
        Ok(())
    }
}

impl AnchorDeserialize for SignatureVerification {
    fn deserialize(buf: &mut &[u8]) -> io::Result<Self> {
        // custodian
        let mut custodian = [0u8; 20];
        buf.read_exact(&mut custodian)?;
        // valid_till
        let mut valid_till = [0u8; 32];
        buf.read_exact(&mut valid_till)?;
        let valid_till = U256::from_little_endian(&valid_till);
        // signature
        let mut signature = vec![0u8; 65];
        buf.read_exact(&mut signature)?;
        Ok(SignatureVerification {
            custodian,
            valid_till,
            signature,
        })
    }
}

#[derive(Clone, Debug)]
pub enum CATSOLStructs {
    Alive { program_id: Pubkey },
    CrossChainPayload { payload: CrossChainStruct },
    SignatureVerification { verification: SignatureVerification },
}

impl AnchorSerialize for CATSOLStructs {
    fn serialize<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            CATSOLStructs::Alive { program_id } => program_id.serialize(writer),
            CATSOLStructs::CrossChainPayload { payload } => payload.serialize(writer),
            CATSOLStructs::SignatureVerification { verification } => verification.serialize(writer),
        }
    }
}

impl AnchorDeserialize for CATSOLStructs {
    fn deserialize(bytes: &mut &[u8]) -> io::Result<Self> {
        // let program_id = Pubkey::deserialize(bytes)?;
        let payload = CrossChainStruct::deserialize(bytes)?;
        // let verification = SignatureVerification::deserialize(bytes)?;
        Ok(CATSOLStructs::CrossChainPayload { payload })
    }
}
