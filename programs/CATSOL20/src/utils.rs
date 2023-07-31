pub mod utils_cat {
    pub fn normalize_amount(amount: u64, decimals: u8) -> u64 {
        if decimals > 8 {
            amount / 10u64.pow((decimals - 8).into())
        } else {
            amount
        }
    }
    
    pub fn denormalize_amount(amount: u64, decimals: u8) -> u64 {
        if decimals > 8 {
            amount * 10u64.pow((decimals - 8).into())
        } else {
            amount
        }
    }
}
