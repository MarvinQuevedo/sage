use indexmap::IndexMap;
use serde::{Deserialize, Serialize};


use crate::Amount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferSummary {
    pub fee: Amount,
    pub maker: OfferAssets,
    pub taker: OfferAssets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferAssets {
    pub xch: OfferXch,
    pub cats: IndexMap<String, OfferCat>,
    pub nfts: IndexMap<String, OfferNft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferXch {
    pub amount: Amount,
    pub royalty: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferCat {
    pub amount: Amount,
    pub royalty: Amount,
    pub name: Option<String>,
    pub ticker: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferNft {
    pub image_data: Option<String>,
    pub image_mime_type: Option<String>,
    pub name: Option<String>,
    pub royalty_percent: String,
}
