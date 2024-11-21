use serde::{Deserialize, Serialize};


use super::NftSortMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCollectionNfts {
    pub collection_id: Option<String>,
    pub offset: u32,
    pub limit: u32,
    pub sort_mode: NftSortMode,
    pub include_hidden: bool,
}
