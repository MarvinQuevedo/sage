use serde::{Deserialize, Serialize};


use crate::Amount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransactionRecord {
    pub transaction_id: String,
    pub fee: Amount,
    pub submitted_at: Option<String>,
}
