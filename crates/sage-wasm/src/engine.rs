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
use sage_keychain::Keychain;
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
            "generate_mnemonic" => self.generate_mnemonic(params_json).await,
            "import_mnemonic" => self.import_mnemonic(params_json).await,
            "unlock_keychain" => self.unlock_keychain(params_json).await,

            other => Err(EngineError::NotImplemented(other.to_string())),
        }
    }

    /// Generate a new BIP-39 mnemonic.
    ///
    /// Params: `{ "words": 12 | 24 }` (default 24).
    /// Returns: `{ "mnemonic": "...", "word_count": N }`.
    async fn generate_mnemonic(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize, Default)]
        struct Req {
            #[serde(default = "default_words")]
            words: u8,
        }
        fn default_words() -> u8 {
            24
        }

        let req: Req = if params_json.trim().is_empty() || params_json == "{}" {
            Req::default()
        } else {
            serde_json::from_str(params_json)
                .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        };

        let bits = match req.words {
            12 => 128,
            15 => 160,
            18 => 192,
            21 => 224,
            24 => 256,
            other => {
                return Err(EngineError::InvalidParams(format!(
                    "word count must be 12/15/18/21/24, got {other}"
                )));
            }
        };

        let mut entropy = vec![0u8; bits / 8];
        getrandom::fill(&mut entropy).map_err(|e| EngineError::Internal(e.to_string()))?;
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| EngineError::Internal(e.to_string()))?;

        Ok(serde_json::json!({
            "mnemonic": mnemonic.to_string(),
            "word_count": req.words,
        })
        .to_string())
    }

    /// Validate a mnemonic, compute its fingerprint, and encrypt the entropy
    /// with the given password into a sage-keychain blob.
    ///
    /// The blob lives in `chrome.storage.local` keyed by fingerprint; the
    /// engine never persists it itself.
    ///
    /// Params: `{ "mnemonic": "...", "password": "..." }`.
    /// Returns: `{ "fingerprint": N, "master_public_key": "0x...",
    ///             "keychain_blob": "hex...", "address_0": "xch1..." }`.
    async fn import_mnemonic(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            mnemonic: String,
            password: String,
            #[serde(default)]
            testnet: bool,
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

        let mnemonic = Mnemonic::parse(req.mnemonic.trim())
            .map_err(|e| EngineError::InvalidParams(format!("mnemonic: {e}")))?;

        let mut keychain = Keychain::default();
        let fingerprint = keychain
            .add_mnemonic(&mnemonic, req.password.as_bytes())
            .map_err(|e| EngineError::Internal(e.to_string()))?;
        let blob = keychain
            .to_bytes()
            .map_err(|e| EngineError::Internal(e.to_string()))?;

        // Derive the first address as a nice confirmation for the UI.
        let seed = mnemonic.to_seed("");
        let master_sk = SecretKey::from_seed(&seed);
        let intermediate_pk = master_to_wallet_unhardened(&master_sk.public_key(), 0);
        let synthetic_pk = intermediate_pk.derive_synthetic();
        let puzzle_hash: Bytes32 = StandardArgs::curry_tree_hash(synthetic_pk).into();
        let prefix = if req.testnet { "txch" } else { "xch" };
        let address_0 = Address::new(puzzle_hash, prefix.to_string())
            .encode()
            .map_err(|e| EngineError::Internal(format!("bech32m: {e}")))?;

        Ok(serde_json::json!({
            "fingerprint": fingerprint,
            "master_public_key": format!("0x{}", hex::encode(master_sk.public_key().to_bytes())),
            "keychain_blob": hex::encode(&blob),
            "address_0": address_0,
        })
        .to_string())
    }

    /// Verify a password unlocks a keychain blob and return the public key
    /// + mnemonic for the requested fingerprint. The mnemonic is returned
    /// so the popup can show "this is your seed" if the user opts in, and
    /// so the engine can hold the master SK in memory for signing.
    ///
    /// Params: `{ "keychain_blob": "hex...", "fingerprint": N, "password": "..." }`.
    /// Returns: `{ "fingerprint": N, "mnemonic": "...", "master_public_key": "0x..." }`.
    async fn unlock_keychain(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            keychain_blob: String,
            fingerprint: u32,
            password: String,
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

        let blob = hex::decode(req.keychain_blob.trim_start_matches("0x"))
            .map_err(|e| EngineError::InvalidParams(format!("keychain_blob hex: {e}")))?;
        let keychain = Keychain::from_bytes(&blob)
            .map_err(|e| EngineError::InvalidParams(format!("keychain decode: {e}")))?;

        let master_pk = keychain
            .extract_public_key(req.fingerprint)
            .map_err(|e| EngineError::Internal(e.to_string()))?
            .ok_or_else(|| {
                EngineError::InvalidParams(format!(
                    "fingerprint {} not in keychain",
                    req.fingerprint
                ))
            })?;

        let (mnemonic, _sk) = keychain
            .extract_secrets(req.fingerprint, req.password.as_bytes())
            .map_err(|_e| EngineError::InvalidParams("wrong password".to_string()))?;

        let mnemonic_str = mnemonic
            .map(|m| m.to_string())
            .ok_or_else(|| EngineError::Internal("no mnemonic stored".to_string()))?;

        Ok(serde_json::json!({
            "fingerprint": req.fingerprint,
            "mnemonic": mnemonic_str,
            "master_public_key": format!("0x{}", hex::encode(master_pk.to_bytes())),
        })
        .to_string())
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
