//! The actual Sage engine — boots and dispatches.
//!
//! Today the dispatch is intentionally narrow:
//! - `version` / `ping` work without any state.
//! - `derive_address` works from a seed phrase + index, no DB needed.
//! - Anything that needs persistent state returns `NotImplemented` until
//!   the JS-callback storage layer lands.

use std::sync::Arc;

use bip39::Mnemonic;
use chia_wallet_sdk::{
    chia::{
        bls::{master_to_wallet_unhardened, PublicKey, SecretKey},
        puzzle_types::{standard::StandardArgs, DeriveSynthetic},
    },
    prelude::*,
    utils::Address,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::{storage_bridge::JsStorage, EngineError};

/// Engine state. Cheap to clone (`Arc`-wrapped storage handle).
#[derive(Clone)]
pub struct SageEngine {
    storage: Arc<JsStorage>,
}

impl SageEngine {
    pub fn new(storage_callbacks: JsValue) -> Result<Self, JsValue> {
        let storage = JsStorage::from_js(storage_callbacks)?;
        Ok(Self {
            storage: Arc::new(storage),
        })
    }

    pub async fn dispatch(&self, method: &str, params_json: &str) -> Result<String, EngineError> {
        match method {
            "ping" => Ok(serde_json::json!({"pong": true}).to_string()),

            "version" => Ok(serde_json::json!({
                "engine": env!("CARGO_PKG_VERSION"),
                "sage_api": "0.12.10",
            })
            .to_string()),

            "derive_address" => self.derive_address(params_json).await,

            other => Err(EngineError::NotImplemented(other.to_string())),
        }
    }

    /// Derive a Chia address from a BIP-39 mnemonic + derivation index.
    /// Pure crypto — no storage involved. Acts as the smoke test that BLS,
    /// puzzle derivation, and bech32m all work in WASM.
    async fn derive_address(&self, params_json: &str) -> Result<String, EngineError> {
        let req: DeriveAddressRequest = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

        let mnemonic = Mnemonic::parse(req.mnemonic.trim())
            .map_err(|e| EngineError::InvalidParams(format!("mnemonic: {e}")))?;
        let seed = mnemonic.to_seed("");

        let master_sk = SecretKey::from_seed(&seed);
        let intermediate_pk: PublicKey = master_to_wallet_unhardened(&master_sk.public_key(), 0);

        let synthetic_pk = intermediate_pk.derive_synthetic();
        let puzzle_hash: Bytes32 = StandardArgs::curry_tree_hash(synthetic_pk).into();
        let prefix = if req.testnet { "txch" } else { "xch" };
        let address = Address::new(puzzle_hash, prefix.to_string())
            .encode()
            .map_err(|e| EngineError::Internal(format!("bech32m: {e}")))?;

        let _ = self.storage.handle(); // silence dead-code for now

        Ok(serde_json::json!({
            "address": address,
            "puzzle_hash": format!("0x{}", hex::encode(puzzle_hash)),
            "public_key": format!("0x{}", hex::encode(synthetic_pk.to_bytes())),
            "index": req.index,
            "testnet": req.testnet,
        })
        .to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DeriveAddressRequest {
    mnemonic: String,
    #[serde(default)]
    index: u32,
    #[serde(default)]
    testnet: bool,
}
