//! The actual Sage engine — boots, dispatches, and holds the unlocked SK.
//!
//! Lifecycle:
//! - `version` / `ping` work without any state.
//! - `generate_mnemonic` / `import_mnemonic` are stateless (no SK yet).
//! - `unlock_keychain` populates the in-memory SK cache for the unlocked
//!   fingerprint. Subsequent `derive_address` / `sign_message` calls use
//!   that cached SK without needing the password again.
//! - `lock_keychain` clears the cache.
//!
//! Anything that needs persistent on-chain state (sync, coin queries)
//! returns `NotImplemented` until the storage bridge is wired.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bip39::Mnemonic;
use chia_wallet_sdk::{
    chia::{
        bls::{master_to_wallet_unhardened, PublicKey, SecretKey, Signature, sign},
        puzzle_types::{standard::StandardArgs, DeriveSynthetic},
    },
    coinset::{ChiaRpcClient, CoinsetClient},
    prelude::*,
    utils::Address,
};
use sage_keychain::Keychain;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::{storage_bridge::JsStorage, EngineError};

/// Engine state. Cheap to clone (`Arc`-wrapped storage + key cache).
#[derive(Clone)]
pub struct SageEngine {
    storage: Arc<JsStorage>,
    /// Master secret keys per fingerprint, populated by `unlock_keychain`.
    /// In WASM/browser this is single-threaded so the Mutex never contends;
    /// it gives us interior mutability + `Clone` for the engine struct.
    unlocked: Arc<Mutex<HashMap<u32, SecretKey>>>,
}

impl SageEngine {
    pub fn new(storage_callbacks: JsValue) -> Result<Self, JsValue> {
        let storage = JsStorage::from_js(storage_callbacks)?;
        Ok(Self {
            storage: Arc::new(storage),
            unlocked: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Look up the cached SK for `fingerprint`, returning a fresh clone.
    fn unlocked_sk(&self, fingerprint: u32) -> Result<SecretKey, EngineError> {
        let guard = self
            .unlocked
            .lock()
            .map_err(|_| EngineError::Internal("unlocked-cache mutex poisoned".to_string()))?;
        guard
            .get(&fingerprint)
            .cloned()
            .ok_or_else(|| {
                EngineError::InvalidParams(format!(
                    "fingerprint {fingerprint} is locked — call unlock_keychain first"
                ))
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
            "derive_addresses" => self.derive_addresses(params_json).await,
            "decode_address" => self.decode_address(params_json).await,
            "generate_mnemonic" => self.generate_mnemonic(params_json).await,
            "import_mnemonic" => self.import_mnemonic(params_json).await,
            "unlock_keychain" => self.unlock_keychain(params_json).await,
            "lock_keychain" => self.lock_keychain(params_json).await,
            "is_unlocked" => self.is_unlocked(params_json).await,
            "sign_message" => self.sign_message(params_json).await,
            "sync_tick" => self.sync_tick(params_json).await,

            other => Err(EngineError::NotImplemented(other.to_string())),
        }
    }

    /// Bulk-derive a range of addresses for the receive screen.
    ///
    /// Params: `{ "fingerprint": N, "start": K, "count": M, "testnet": bool }`.
    /// Returns: `{ "addresses": [{ index, address, puzzle_hash, public_key }] }`.
    async fn derive_addresses(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            fingerprint: u32,
            #[serde(default)]
            start: u32,
            #[serde(default = "default_count")]
            count: u32,
            #[serde(default)]
            testnet: bool,
        }
        fn default_count() -> u32 {
            10
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        if req.count == 0 || req.count > 200 {
            return Err(EngineError::InvalidParams(format!(
                "count must be 1..=200, got {}",
                req.count
            )));
        }
        let master_pk = self.unlocked_sk(req.fingerprint)?.public_key();
        let prefix = if req.testnet { "txch" } else { "xch" };
        let mut out = Vec::with_capacity(req.count as usize);
        for i in 0..req.count {
            let idx = req.start + i;
            let intermediate_pk = master_to_wallet_unhardened(&master_pk, idx);
            let synthetic_pk = intermediate_pk.derive_synthetic();
            let puzzle_hash: Bytes32 = StandardArgs::curry_tree_hash(synthetic_pk).into();
            let address = Address::new(puzzle_hash, prefix.to_string())
                .encode()
                .map_err(|e| EngineError::Internal(format!("bech32m: {e}")))?;
            out.push(serde_json::json!({
                "index": idx,
                "address": address,
                "puzzle_hash": format!("0x{}", hex::encode(puzzle_hash)),
                "public_key": format!("0x{}", hex::encode(synthetic_pk.to_bytes())),
            }));
        }
        Ok(serde_json::json!({ "addresses": out }).to_string())
    }

    /// Parse a bech32m Chia address into its puzzle hash + prefix.
    ///
    /// Params: `{ "address": "xch1..." }`.
    /// Returns: `{ "puzzle_hash": "0x...", "prefix": "xch" | "txch" }`.
    async fn decode_address(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            address: String,
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        let parsed = Address::decode(req.address.trim())
            .map_err(|e| EngineError::InvalidParams(format!("bech32m: {e}")))?;
        Ok(serde_json::json!({
            "puzzle_hash": format!("0x{}", hex::encode(parsed.puzzle_hash)),
            "prefix": parsed.prefix,
        })
        .to_string())
    }

    /// One sync poll against the configured Chia RPC backend.
    ///
    /// Today this is a smoke test: hit `get_blockchain_state` against the
    /// mainnet coinset.org endpoint and return the current peak height +
    /// sync mode + network info. Real wallet sync (per-puzzle-hash polling,
    /// hint walking, mempool watch) will come next on top of this.
    ///
    /// Params: `{ "endpoint"?: "mainnet" | "testnet11" | "<url>" }` (default
    /// mainnet).
    async fn sync_tick(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize, Default)]
        struct Req {
            #[serde(default)]
            endpoint: Option<String>,
        }
        let req: Req = if params_json.trim().is_empty() || params_json == "{}" {
            Req::default()
        } else {
            serde_json::from_str(params_json)
                .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        };
        let client = match req.endpoint.as_deref() {
            None | Some("mainnet") => CoinsetClient::mainnet(),
            Some("testnet11") => CoinsetClient::testnet11(),
            Some(url) => CoinsetClient::new(url.to_string()),
        };
        let state = client
            .get_blockchain_state()
            .await
            .map_err(|e| EngineError::Internal(format!("coinset rpc: {e}")))?;
        let body = state.blockchain_state.ok_or_else(|| {
            EngineError::Internal(state.error.unwrap_or_else(|| "empty response".to_string()))
        })?;
        Ok(serde_json::json!({
            "peak_height": body.peak.height,
            "peak_header_hash": format!("0x{}", hex::encode(body.peak.header_hash)),
            "synced": body.sync.synced,
            "sync_mode": body.sync.sync_mode,
            "mempool_size": body.mempool_size,
            "mempool_cost": body.mempool_cost,
            "difficulty": body.difficulty,
        })
        .to_string())
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

        match req.words {
            12 | 15 | 18 | 21 | 24 => {}
            other => {
                return Err(EngineError::InvalidParams(format!(
                    "word count must be 12/15/18/21/24, got {other}"
                )));
            }
        }

        // `bip39::Mnemonic::generate` uses `rand`'s `thread_rng()` under the
        // hood, which is satisfied by `rand`'s `getrandom` backend selection.
        // We've already declared the proper getrandom features for wasm32 in
        // sage-wallet's target-conditional deps; native uses the OS RNG.
        let mnemonic = Mnemonic::generate(req.words as usize)
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

        let (mnemonic, sk) = keychain
            .extract_secrets(req.fingerprint, req.password.as_bytes())
            .map_err(|_e| EngineError::InvalidParams("wrong password".to_string()))?;

        let mnemonic_str = mnemonic
            .map(|m| m.to_string())
            .ok_or_else(|| EngineError::Internal("no mnemonic stored".to_string()))?;

        // Cache the master SK so subsequent derive/sign calls don't need the
        // password again for the rest of this engine's lifetime.
        if let Some(sk) = sk {
            let mut guard = self
                .unlocked
                .lock()
                .map_err(|_| EngineError::Internal("unlocked-cache mutex poisoned".to_string()))?;
            guard.insert(req.fingerprint, sk);
        }

        Ok(serde_json::json!({
            "fingerprint": req.fingerprint,
            "mnemonic": mnemonic_str,
            "master_public_key": format!("0x{}", hex::encode(master_pk.to_bytes())),
        })
        .to_string())
    }

    /// Derive a Chia address.
    ///
    /// Two modes:
    /// * `{ "fingerprint": N, "index": K, "testnet": bool }` — uses the
    ///   unlocked SK cached at `unlock_keychain` time. Preferred.
    /// * `{ "mnemonic": "...", "index": K, "testnet": bool }` — pure stateless
    ///   path that re-derives from a mnemonic. Kept for one-off lookups.
    async fn derive_address(&self, params_json: &str) -> Result<String, EngineError> {
        let req: DeriveAddressRequest = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

        let master_pk: PublicKey = if let Some(fp) = req.fingerprint {
            self.unlocked_sk(fp)?.public_key()
        } else if let Some(mnemonic_str) = req.mnemonic.as_deref() {
            let mnemonic = Mnemonic::parse(mnemonic_str.trim())
                .map_err(|e| EngineError::InvalidParams(format!("mnemonic: {e}")))?;
            SecretKey::from_seed(&mnemonic.to_seed("")).public_key()
        } else {
            return Err(EngineError::InvalidParams(
                "derive_address requires either `fingerprint` (preferred) or `mnemonic`"
                    .to_string(),
            ));
        };

        let intermediate_pk: PublicKey = master_to_wallet_unhardened(&master_pk, req.index);
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

    /// Clear the cached SK for one fingerprint (or all if `fingerprint`
    /// omitted). Called when the user locks the popup.
    async fn lock_keychain(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize, Default)]
        struct Req {
            fingerprint: Option<u32>,
        }
        let req: Req = if params_json.trim().is_empty() || params_json == "{}" {
            Req::default()
        } else {
            serde_json::from_str(params_json)
                .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        };

        let mut guard = self
            .unlocked
            .lock()
            .map_err(|_| EngineError::Internal("unlocked-cache mutex poisoned".to_string()))?;
        match req.fingerprint {
            Some(fp) => {
                guard.remove(&fp);
            }
            None => guard.clear(),
        }

        Ok(serde_json::json!({ "locked": true }).to_string())
    }

    /// Report whether a given fingerprint is currently unlocked in this
    /// engine instance.
    async fn is_unlocked(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            fingerprint: u32,
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        let guard = self
            .unlocked
            .lock()
            .map_err(|_| EngineError::Internal("unlocked-cache mutex poisoned".to_string()))?;
        Ok(serde_json::json!({
            "fingerprint": req.fingerprint,
            "unlocked": guard.contains_key(&req.fingerprint),
        })
        .to_string())
    }

    /// BLS-sign a message with a derived key.
    ///
    /// `message` is hex (with or without leading `0x`). The signing key is
    /// the synthetic key at `index` for the unlocked fingerprint — i.e. the
    /// same key that owns the address at that index. Uses the standard
    /// BLS augmented scheme (`sign`).
    async fn sign_message(&self, params_json: &str) -> Result<String, EngineError> {
        #[derive(Deserialize)]
        struct Req {
            fingerprint: u32,
            #[serde(default)]
            index: u32,
            message: String,
        }
        let req: Req = serde_json::from_str(params_json)
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;

        let master_sk = self.unlocked_sk(req.fingerprint)?;
        let intermediate_sk = master_to_wallet_unhardened(&master_sk, req.index);
        let synthetic_sk = intermediate_sk.derive_synthetic();

        let bytes = hex::decode(req.message.trim_start_matches("0x"))
            .map_err(|e| EngineError::InvalidParams(format!("message hex: {e}")))?;
        let signature: Signature = sign(&synthetic_sk, &bytes);

        Ok(serde_json::json!({
            "signature": format!("0x{}", hex::encode(signature.to_bytes())),
            "public_key": format!("0x{}", hex::encode(synthetic_sk.public_key().to_bytes())),
            "index": req.index,
        })
        .to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DeriveAddressRequest {
    #[serde(default)]
    fingerprint: Option<u32>,
    #[serde(default)]
    mnemonic: Option<String>,
    #[serde(default)]
    index: u32,
    #[serde(default)]
    testnet: bool,
}
