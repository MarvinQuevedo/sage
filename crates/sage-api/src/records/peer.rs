use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub ip_addr: String,
    pub port: u16,
    pub trusted: bool,
    pub peak_height: u32,
}
