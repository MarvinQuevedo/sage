use serde::{Deserialize, Serialize};


use crate::Amount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeOffer {
    pub requested_assets: Assets,
    pub offered_assets: Assets,
    pub fee: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assets {
    pub xch: Amount,
    pub cats: Vec<CatAmount>,
    pub nfts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatAmount {
    pub asset_id: String,
    pub amount: Amount,
}
