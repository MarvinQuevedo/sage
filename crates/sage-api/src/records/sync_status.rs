use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{Amount, Unit};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncStatus {
    pub balance: Amount,
    pub unit: Unit,
    pub synced_coins: u32,
    pub total_coins: u32,
    pub receive_address: String,
    pub burn_address: String,
}
