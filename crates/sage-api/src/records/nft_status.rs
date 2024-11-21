use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NftStatus {
    pub nfts: u32,
    pub visible_nfts: u32,
    pub collections: u32,
    pub visible_collections: u32,
}
