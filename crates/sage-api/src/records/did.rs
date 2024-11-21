use serde::{Deserialize, Serialize};


use crate::Amount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidRecord {
    pub launcher_id: String,
    pub name: Option<String>,
    pub visible: bool,
    pub coin_id: String,
    pub address: String,
    pub amount: Amount,
    pub created_height: Option<u32>,
    pub create_transaction_id: Option<String>,
}
