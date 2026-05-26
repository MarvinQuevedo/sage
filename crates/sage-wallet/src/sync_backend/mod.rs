//! Pluggable sync backend.
//!
//! Sage's original sync layer is hardcoded to the Chia P2P wallet protocol via
//! [`WalletPeer`] (see `src/wallet_peer.rs`). For targets that cannot speak
//! P2P over self-signed TLS — primarily the browser extension — we need a
//! second implementation that fetches the same data over HTTPS from a Chia
//! full-node RPC provider such as `api.coinset.org`.
//!
//! This module introduces a [`SyncBackend`] trait that captures the read /
//! write surface the rest of `sage-wallet` already uses against
//! [`WalletPeer`]. Two implementations live alongside it:
//!
//! * [`peer::PeerBackend`] — wraps a [`WalletPeer`] one-to-one (no behavior
//!   change for desktop/mobile builds).
//! * [`coinset::CoinsetBackend`] — HTTP client targeting Chia's full-node RPC
//!   surface (gated behind the `coinset-sync` cargo feature).
//!
//! The integration into [`crate::SyncManager`] is intentionally NOT part of
//! this commit — the manager continues to drive `WalletPeer` directly. A later
//! refactor will introduce `SyncManager::new(Arc<dyn SyncBackend>, ...)` and
//! remove the direct coupling.

use chia_wallet_sdk::{
    chia::protocol::{
        CoinStateFilters, RespondPeers, RespondPuzzleState, TransactionAck,
    },
    prelude::*,
};

use crate::WalletError;

#[cfg(not(target_arch = "wasm32"))]
pub mod peer;
#[cfg(feature = "coinset-sync")]
pub mod coinset;

/// Read/write surface required by the wallet sync loop.
///
/// All methods are async and `Send`-friendly so the trait object can be driven
/// from a Tokio runtime (native) or a `wasm-bindgen-futures` executor
/// (browser).
///
/// Subscription methods (`subscribe_*`, `unsubscribe_*`) have peer-protocol
/// semantics. On HTTP backends without push (coinset.org) they are no-ops or
/// emulated with polling state held inside the backend — callers should not
/// assume real-time pushes are available.
pub trait SyncBackend: Send + Sync + 'static {
    fn fetch_coin(
        &self,
        coin_id: Bytes32,
        genesis_challenge: Bytes32,
    ) -> impl Future<Output = Result<CoinState, WalletError>> + Send;

    fn fetch_optional_coin(
        &self,
        coin_id: Bytes32,
        genesis_challenge: Bytes32,
    ) -> impl Future<Output = Result<Option<CoinState>, WalletError>> + Send;

    fn fetch_coins(
        &self,
        coin_ids: Vec<Bytes32>,
        genesis_challenge: Bytes32,
    ) -> impl Future<Output = Result<Vec<CoinState>, WalletError>> + Send;

    fn fetch_puzzle_solution(
        &self,
        coin_id: Bytes32,
        spent_height: u32,
    ) -> impl Future<Output = Result<(Program, Program), WalletError>> + Send;

    fn fetch_coin_spend(
        &self,
        coin_id: Bytes32,
        genesis_challenge: Bytes32,
    ) -> impl Future<Output = Result<CoinSpend, WalletError>> + Send;

    fn fetch_optional_coin_spend(
        &self,
        coin_id: Bytes32,
        genesis_challenge: Bytes32,
    ) -> impl Future<Output = Result<Option<CoinSpend>, WalletError>> + Send;

    fn try_fetch_singleton_child(
        &self,
        coin_id: Bytes32,
    ) -> impl Future<Output = Result<Option<CoinState>, WalletError>> + Send;

    fn send_transaction(
        &self,
        spend_bundle: SpendBundle,
    ) -> impl Future<Output = Result<TransactionAck, WalletError>> + Send;

    fn unsubscribe(&self) -> impl Future<Output = Result<(), WalletError>> + Send;

    fn subscribe_coins(
        &self,
        coin_ids: Vec<Bytes32>,
        previous_height: Option<u32>,
        header_hash: Bytes32,
    ) -> impl Future<Output = Result<Vec<CoinState>, WalletError>> + Send;

    fn subscribe_puzzles(
        &self,
        puzzle_hashes: Vec<Bytes32>,
        previous_height: Option<u32>,
        header_hash: Bytes32,
        filters: CoinStateFilters,
    ) -> impl Future<Output = Result<RespondPuzzleState, WalletError>> + Send;

    fn unsubscribe_coins(
        &self,
        coin_ids: Vec<Bytes32>,
    ) -> impl Future<Output = Result<(), WalletError>> + Send;

    fn block_timestamp(
        &self,
        height: u32,
    ) -> impl Future<Output = Result<(Bytes32, u64), WalletError>> + Send;

    /// Peer-only — returns an empty list on HTTP backends.
    fn request_peers(
        &self,
    ) -> impl Future<Output = Result<RespondPeers, WalletError>> + Send;
}
