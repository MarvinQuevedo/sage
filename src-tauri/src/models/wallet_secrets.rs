use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSecrets {
    pub mnemonic: Option<String>,
    pub secret_key: String,
}
