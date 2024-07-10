use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wallets: IndexMap<String, WalletConfig>,
    pub active_wallet: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    pub name: String,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            name: "Unnamed Wallet".to_string(),
        }
    }
}