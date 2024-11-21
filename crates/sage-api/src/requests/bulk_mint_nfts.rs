use serde::{Deserialize, Serialize};


use crate::{Amount, TransactionSummary};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkMintNfts {
    pub nft_mints: Vec<NftMint>,
    pub did_id: String,
    pub fee: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftMint {
    pub edition_number: Option<u32>,
    pub edition_total: Option<u32>,
    pub data_uris: Vec<String>,
    pub metadata_uris: Vec<String>,
    pub license_uris: Vec<String>,
    pub royalty_address: Option<String>,
    pub royalty_percent: Amount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkMintNftsResponse {
    pub nft_ids: Vec<String>,
    pub summary: TransactionSummary,
}
