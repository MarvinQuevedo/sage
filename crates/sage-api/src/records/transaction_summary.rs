use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{Amount, AssetKind};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TransactionSummary {
    pub fee: Amount,
    pub inputs: Vec<TransactionInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpendBundleJson {
    pub coin_spends: Vec<CoinSpendJson>,
    pub aggregated_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CoinSpendJson {
    pub coin: CoinJson,
    pub puzzle_reveal: String,
    pub solution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CoinJson {
    pub parent_coin_info: String,
    pub puzzle_hash: String,
    pub amount: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TransactionInput {
    pub coin_id: String,
    pub amount: Amount,
    pub address: String,
    #[serde(flatten)]
    pub kind: AssetKind,
    pub outputs: Vec<TransactionOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TransactionOutput {
    pub coin_id: String,
    pub amount: Amount,
    pub address: String,
    pub receiving: bool,
    pub burning: bool,
}
