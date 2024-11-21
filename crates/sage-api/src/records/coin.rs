use serde::{Deserialize, Serialize};


use crate::Amount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinRecord {
    pub coin_id: String,
    pub address: String,
    pub amount: Amount,
    pub created_height: Option<u32>,
    pub spent_height: Option<u32>,
    pub create_transaction_id: Option<String>,
    pub spend_transaction_id: Option<String>,
}
