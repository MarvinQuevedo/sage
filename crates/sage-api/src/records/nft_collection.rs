use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftCollectionRecord {
    pub collection_id: String,
    pub did_id: String,
    pub metadata_collection_id: String,
    pub visible: bool,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub nfts: u32,
    pub visible_nfts: u32,
}
