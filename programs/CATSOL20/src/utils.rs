pub mod utils_cat {

    pub fn normalize_amount(
        mut amount: u64,
        foreign_decimals: u8,
        local_decimals: u8,
    ) -> Option<u64> {

        if foreign_decimals > local_decimals {
            let diff = foreign_decimals - local_decimals;
            amount /= 10u64.pow(diff.into());

        } else if foreign_decimals < local_decimals {

            let diff = local_decimals - foreign_decimals;
            for _ in 0..diff {
                match amount.checked_mul(10) {
                    Some(val) => amount = val,
                    None => return None,
                }
            }
        
        }
        Some(amount)
    }
    
}
