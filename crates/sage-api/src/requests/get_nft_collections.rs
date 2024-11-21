use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GetNftCollections {
    pub offset: u32,
    pub limit: u32,
    pub include_hidden: bool,
}
