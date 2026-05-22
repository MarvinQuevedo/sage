use std::{sync::Arc, time::Duration};

use anyhow::{Result, bail};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use bip39::Mnemonic;
use chia_wallet_sdk::{
    chia::{
        bls::master_to_wallet_unhardened,
        puzzle_types::{DeriveSynthetic, standard::StandardArgs},
    },
    prelude::*,
    puzzles::SETTLEMENT_PAYMENT_HASH,
    test::PeerSimulator,
    types::puzzles::P2DelegatedConditionsArgs,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rustls::crypto::aws_lc_rs::default_provider;
use sage::Sage;
use sage_api::{
    Amount, CoinSpendJson, GetCats, GetKey, GetNfts, GetOffers, GetPeers, GetSecretKey,
    GetSyncStatus, GetVersion, ImportKey, Login, MakeOffer, MakeOfferUnsigned, NftSortMode,
    OfferAmount, RekeyKeychain, RequiredSignatures, SendCat, SendXch, TransferNfts, UnlockKeychain,
};
use sage_api_macro::impl_endpoints;
use sage_wallet::{SyncCommand, SyncEvent};
use serde::{Serialize, de::DeserializeOwned};
use tempfile::TempDir;
use tokio::{
    sync::{Mutex, mpsc},
    time::timeout,
};
use tower::ServiceExt;
use tracing::debug;

use crate::make_router;

struct TestApp {
    sage: Arc<Mutex<Sage>>,
    router: Router<()>,
    rng: ChaCha8Rng,
    sim: PeerSimulator,
    events: mpsc::Receiver<SyncEvent>,
    _dir: TempDir,
}

impl TestApp {
    pub async fn new() -> Result<Self> {
        let _ = default_provider().install_default();

        let dir = TempDir::new()?;
        let rng = ChaCha8Rng::seed_from_u64(1337);
        let sim = PeerSimulator::new().await?;

        let mut sage = Sage::new(dir.path(), true);

        // Make sure we don't attempt to connect to actual nodes
        sage.config.network.target_peers = 1;
        sage.config.network.discover_peers = false;
        sage.config.network.default_network = "testnet11".to_string();

        let events = sage.initialize().await?;

        let sage = Arc::new(Mutex::new(sage));
        let router = make_router(sage.clone());

        let app = Self {
            sage,
            router,
            rng,
            sim,
            events,
            _dir: dir,
        };

        let (peer, receiver) = app.sim.connect_raw().await?;

        app.sage
            .lock()
            .await
            .command_sender
            .send(SyncCommand::AddPeer { peer, receiver })
            .await?;

        Ok(app)
    }

    /// Like [`TestApp::new`] but connects to **real Chia mainnet peers**
    /// (DNS-discovered) instead of the local simulator. Used by the live
    /// Tangem diagnostic (`#[ignore]`d; requires network access).
    pub async fn new_mainnet() -> Result<Self> {
        let _ = default_provider().install_default();

        let dir = TempDir::new()?;
        let rng = ChaCha8Rng::seed_from_u64(1337);
        let sim = PeerSimulator::new().await?; // created but unused on mainnet

        let mut sage = Sage::new(dir.path(), true);

        sage.config.network.target_peers = 5;
        sage.config.network.discover_peers = true;
        sage.config.network.default_network = "mainnet".to_string();

        let events = sage.initialize().await?;

        let sage = Arc::new(Mutex::new(sage));
        let router = make_router(sage.clone());

        // No AddPeer: the sync manager discovers mainnet peers via the
        // network's DNS introducers.
        Ok(Self {
            sage,
            router,
            rng,
            sim,
            events,
            _dir: dir,
        })
    }

    async fn call_rpc<T: Serialize, R: DeserializeOwned>(&self, path: &str, body: T) -> Result<R> {
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body)?))?;

        let response = self.router.clone().oneshot(req).await?;
        let status = response.status();

        if status != StatusCode::OK {
            let body = response.into_body();
            let body = axum::body::to_bytes(body, usize::MAX).await?;
            bail!(
                "RPC request failed with status {status}: {}",
                String::from_utf8(body.to_vec())?
            );
        }

        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await?;

        Ok(serde_json::from_slice(&body)?)
    }

    async fn setup_bls(&mut self, balance: u64) -> Result<u32> {
        let mnemonic = Mnemonic::from_entropy(&self.rng.r#gen::<[u8; 16]>())?;

        if balance > 0 {
            let master_sk = SecretKey::from_seed(&mnemonic.to_seed(""));
            let p2_puzzle_hash = StandardArgs::curry_tree_hash(
                master_to_wallet_unhardened(&master_sk, 0)
                    .public_key()
                    .derive_synthetic(),
            );

            self.sim.lock().await.create_block();

            self.sim
                .lock()
                .await
                .new_coin(p2_puzzle_hash.into(), balance);
        }

        let fingerprint = self
            .import_key(ImportKey {
                name: "Alice".to_string(),
                key: mnemonic.to_string(),
                derivation_index: 0,
                hardened: None,
                unhardened: None,
                save_secrets: true,
                login: true,
                emoji: None,
                arbor_only: false,
            })
            .await?
            .fingerprint;

        self.consume_until(|event| matches!(event, SyncEvent::Subscribed))
            .await;

        Ok(fingerprint)
    }

    async fn consume_until(&mut self, f: impl Fn(SyncEvent) -> bool) {
        loop {
            let next = timeout(Duration::from_secs(10), self.events.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out listening for event"))
                .unwrap_or_else(|| panic!("missing next event"));

            debug!("Consuming event: {next:?}");

            if f(next) {
                return;
            }
        }
    }

    async fn wait_for_coins(&mut self) {
        self.consume_until(|event| matches!(event, SyncEvent::CoinsUpdated))
            .await;
    }

    #[allow(unused)]
    async fn wait_for_puzzles(&mut self) {
        self.consume_until(|event| matches!(event, SyncEvent::PuzzleBatchSynced))
            .await;
    }

    /// Imports a Tangem card as a watch-only wallet: only the card's BLS
    /// public key is known. Sage curries it into the `p2_delegated_conditions`
    /// ("arbor") puzzle — the exact puzzle a Tangem card spends. Optionally
    /// funds the arbor puzzle hash with `balance` mojos of XCH.
    ///
    /// Returns `(fingerprint, card_public_key, arbor_puzzle_hash)`.
    async fn setup_tangem(&mut self, balance: u64) -> Result<(u32, PublicKey, Bytes32)> {
        let card_pk = PublicKey::from_bytes(&TANGEM_PUBLIC_KEY)?;
        let arbor_ph: Bytes32 = P2DelegatedConditionsArgs::new(card_pk)
            .curry_tree_hash()
            .into();

        if balance > 0 {
            self.sim.lock().await.create_block();
            self.sim.lock().await.new_coin(arbor_ph, balance);
        }

        // External-signer import: ONLY the card's BLS public key. `arbor_only`
        // makes Sage create exactly one `p2_delegated_conditions` puzzle and
        // ZERO HD derivations (derivation_index/hardened/unhardened ignored).
        let fingerprint = self
            .import_key(ImportKey {
                name: "Tangem".to_string(),
                key: TANGEM_PUBLIC_KEY_HEX.to_string(),
                derivation_index: 0,
                hardened: None,
                unhardened: None,
                save_secrets: false,
                login: true,
                emoji: None,
                arbor_only: true,
            })
            .await?
            .fingerprint;

        Ok((fingerprint, card_pk, arbor_ph))
    }

    /// Drain any buffered sync events (the channel has a bounded buffer; we
    /// don't assert on event order for the Tangem tests, we poll state).
    fn drain_events(&mut self) {
        while self.events.try_recv().is_ok() {}
    }
}

/// The real Tangem card BLS public key (48-byte G1) provided by the user.
const TANGEM_PUBLIC_KEY: [u8; 48] = hex_literal::hex!(
    "8fba5482e6c798a06ee1fd95deaaa83f11c46da06006ab3524e917f4e116c2bdec69d6098043ca568290ac366e5e2dc5"
);
const TANGEM_PUBLIC_KEY_HEX: &str =
    "0x8fba5482e6c798a06ee1fd95deaaa83f11c46da06006ab3524e917f4e116c2bdec69d6098043ca568290ac366e5e2dc5";
/// The single puzzle hash + mainnet address the user confirmed for this card.
const TANGEM_ARBOR_PUZZLE_HASH: &str =
    "bdca1be3075afcf8b7fdf3c0bbfee3341c439d9d7d44cc798d3e0ca66bb42389";
const TANGEM_ARBOR_ADDRESS_MAINNET: &str =
    "xch1hh9phcc8tt703dla70qthlhrxswy88va04zvc7vd8cx2v6a5ywyst8mgul";

impl_endpoints! {
    impl TestApp {
        (repeat pub async fn endpoint(&self, body: sage_api::Endpoint) -> Result<sage_api::EndpointResponse> {
            self.call_rpc(&format!("/{}", endpoint_string), body).await
        })
    }
}

#[tokio::test]
async fn test_rpc_version() -> Result<()> {
    let app = TestApp::new().await?;

    let response = app.get_version(GetVersion {}).await?;

    assert_eq!(response.version, env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio::test]
async fn test_initial_state() -> Result<()> {
    let mut app = TestApp::new().await?;

    let fingerprint = app.setup_bls(0).await?;

    let key = app
        .get_key(GetKey { fingerprint: None })
        .await?
        .key
        .expect("should be logged in");

    assert_eq!(key.fingerprint, fingerprint);

    let peers = app.get_peers(GetPeers {}).await?.peers;

    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].peak_height, 0);
    assert!(!peers[0].user_managed);

    let status = app.get_sync_status(GetSyncStatus {}).await?;

    assert_eq!(status.synced_coins, 0);
    assert_eq!(status.total_coins, 0);
    assert_eq!(status.selectable_balance.to_u64(), Some(0));
    assert_eq!(status.unhardened_derivation_index, 1000);
    assert_eq!(status.hardened_derivation_index, 0);
    assert_eq!(
        status.receive_address,
        "txch19hutewzq3z4l6y3fsw5laatre79tuz5p43jlvag0yz466xx9l7vs4vnpem"
    );

    Ok(())
}

#[tokio::test]
async fn test_send_xch() -> Result<()> {
    let mut app = TestApp::new().await?;

    let alice = app.setup_bls(1000).await?;

    let bob = app.setup_bls(1000).await?;
    let bob_address = app.get_sync_status(GetSyncStatus {}).await?.receive_address;

    app.login(Login { fingerprint: alice }).await?;

    let balance = app
        .get_sync_status(GetSyncStatus {})
        .await?
        .selectable_balance
        .to_u64();
    assert_eq!(balance, Some(1000));

    app.wait_for_coins().await;

    app.send_xch(SendXch {
        address: bob_address,
        amount: Amount::u64(1000),
        fee: Amount::u64(0),
        memos: vec![],
        clawback: None,
        auto_submit: true,
    })
    .await?;

    app.wait_for_coins().await;

    let balance = app
        .get_sync_status(GetSyncStatus {})
        .await?
        .selectable_balance
        .to_u64();
    assert_eq!(balance, Some(0));

    app.login(Login { fingerprint: bob }).await?;

    app.wait_for_coins().await;

    let balance = app
        .get_sync_status(GetSyncStatus {})
        .await?
        .selectable_balance
        .to_u64();
    assert_eq!(balance, Some(2000));

    Ok(())
}

// ---------------------------------------------------------------------------
// Tangem / arbor (external-signer) tests
//
// Run with output:
//   SQLX_OFFLINE=true cargo test -p sage-rpc tangem -- --nocapture
// ---------------------------------------------------------------------------

/// Importing only the card's public key (arbor_only) must produce EXACTLY the
/// single `p2_delegated_conditions` puzzle hash + address the user confirmed
/// for this real Tangem card, and zero HD derivations.
#[tokio::test]
async fn test_tangem_import_and_address() -> Result<()> {
    let mut app = TestApp::new().await?;

    let (fingerprint, card_pk, arbor_ph) = app.setup_tangem(0).await?;

    // The single puzzle hash matches the value the user confirmed on-chain.
    assert_eq!(hex::encode(arbor_ph), TANGEM_ARBOR_PUZZLE_HASH);

    // ...and its mainnet address matches.
    let mainnet = Address::new(arbor_ph, "xch".to_string()).encode()?;
    assert_eq!(mainnet, TANGEM_ARBOR_ADDRESS_MAINNET);

    // The wallet is logged in and its single receive == change == arbor addr.
    let status = app.get_sync_status(GetSyncStatus {}).await?;
    let testnet_arbor = Address::new(arbor_ph, "txch".to_string()).encode()?;
    assert_eq!(status.receive_address, testnet_arbor);

    let key = app
        .get_key(GetKey { fingerprint: None })
        .await?
        .key
        .expect("logged in");
    assert_eq!(key.fingerprint, fingerprint);
    assert!(!key.has_secrets, "Tangem wallet must not store secrets");

    println!("\n=== TANGEM import ===");
    println!("card public key : {}", hex::encode(card_pk.to_bytes()));
    println!("arbor puzzle    : 0x{}", hex::encode(arbor_ph));
    println!("mainnet address : {mainnet}");
    println!("testnet address : {testnet_arbor}");
    println!("has_secrets     : {}", key.has_secrets);

    Ok(())
}

/// LIVE diagnostic against real Chia **mainnet** peers (no simulator).
///
///   SQLX_OFFLINE=true cargo test -p sage-rpc \
///       test_tangem_mainnet_live -- --ignored --nocapture
///
/// Imports ONLY the card's master public key (arbor_only), syncs the single
/// `p2_delegated_conditions` puzzle from mainnet, prints the real balance /
/// CATs / NFTs, and builds UNSIGNED spend bundles + the messages the card
/// must sign for whatever the wallet actually holds.
#[tokio::test]
#[ignore = "requires mainnet network access"]
async fn test_tangem_mainnet_live() -> Result<()> {
    let mut app = TestApp::new_mainnet().await?;

    let (_fp, card_pk, arbor_ph) = app.setup_tangem(0).await?;
    let address = Address::new(arbor_ph, "xch".to_string()).encode()?;

    println!("\n=== TANGEM mainnet live ===");
    println!("card public key : {}", hex::encode(card_pk.to_bytes()));
    println!("arbor puzzle    : 0x{}", hex::encode(arbor_ph));
    println!("mainnet address : {address}");

    // Wait (up to ~120s) for the sync manager to discover mainnet peers.
    let mut connected = false;
    for _ in 0..120 {
        app.drain_events();
        if let Ok(p) = app.get_peers(GetPeers {}).await {
            let best = p.peers.iter().map(|x| x.peak_height).max().unwrap_or(0);
            if !p.peers.is_empty() && best > 0 {
                println!("peers           : {} (peak {})", p.peers.len(), best);
                connected = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if !connected {
        println!("!! no mainnet peers reachable (network blocked?) — aborting live probe");
        return Ok(());
    }

    // Let the single arbor puzzle subscription settle against the node.
    for _ in 0..45 {
        app.drain_events();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let status = app.get_sync_status(GetSyncStatus {}).await?;
    println!("receive address : {}", status.receive_address);
    println!(
        "xch balance     : {:?} mojos  (synced {} / {} coins)",
        status.selectable_balance.to_u64(),
        status.synced_coins,
        status.total_coins
    );
    let cats = app.get_cats(GetCats {}).await?.cats;
    println!("cats            : {cats:?}");
    let nfts = app
        .get_nfts(GetNfts {
            collection_id: None,
            minter_did_id: None,
            owner_did_id: None,
            name: None,
            offset: 0,
            limit: 100,
            sort_mode: NftSortMode::Recent,
            include_hidden: true,
        })
        .await?
        .nfts;
    println!("nfts            : {}", nfts.len());

    // Helper: dump the messages the card must sign for a built tx.
    async fn dump_required(app: &TestApp, label: &str, coin_spends: Vec<sage_api::CoinSpendJson>) {
        if coin_spends.is_empty() {
            return;
        }
        let req = app
            .required_signatures(RequiredSignatures {
                coin_spends: coin_spends.clone(),
            })
            .await
            .expect("required_signatures");
        println!("--- {label}: {} coin spend(s) ---", coin_spends.len());
        for (i, s) in req.signatures.iter().enumerate() {
            assert_eq!(
                s.public_key, TANGEM_PUBLIC_KEY_HEX,
                "every signature must be the card key"
            );
            println!("  sig[{i}] msg   : {}", s.message);
        }
    }

    // XCH: build an unsigned self-send of the whole balance.
    if let Some(bal) = status.selectable_balance.to_u64()
        && bal > 0
    {
        let tx = app
            .send_xch(SendXch {
                address: address.clone(),
                amount: Amount::u64(bal),
                fee: Amount::u64(0),
                memos: vec![],
                clawback: None,
                auto_submit: false,
            })
            .await?;
        dump_required(&app, "XCH send (unsigned)", tx.coin_spends).await;
    } else {
        println!("(no XCH to spend)");
    }

    // CATs: build an unsigned self-send for each.
    for cat in &cats {
        let Some(asset_id) = cat.asset_id.clone() else {
            continue;
        };
        match app
            .send_cat(SendCat {
                asset_id: asset_id.clone(),
                address: address.clone(),
                amount: Amount::u64(1),
                fee: Amount::u64(0),
                include_hint: true,
                memos: vec![],
                clawback: None,
                auto_submit: false,
            })
            .await
        {
            Ok(tx) => dump_required(&app, &format!("CAT {asset_id} (unsigned)"), tx.coin_spends).await,
            Err(e) => println!("(CAT {asset_id} send build failed: {e})"),
        }
    }

    // NFTs: build an unsigned self-transfer for each.
    for nft in &nfts {
        match app
            .transfer_nfts(TransferNfts {
                nft_ids: vec![nft.launcher_id.clone()],
                address: address.clone(),
                fee: Amount::u64(0),
                clawback: None,
                auto_submit: false,
            })
            .await
        {
            Ok(tx) => {
                dump_required(&app, &format!("NFT {} (unsigned)", nft.launcher_id), tx.coin_spends)
                    .await
            }
            Err(e) => println!("(NFT {} transfer build failed: {e})", nft.launcher_id),
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tangem unsigned-build verification (simulator, no network)
//
//   SQLX_OFFLINE=true cargo test -p sage-rpc tangem_unsigned -- --nocapture
//
// Proves the observer wallet (ONLY the card's public key) can build spend
// bundles that actually RUN through the CLVM, so the Tangem card just adds the
// signature afterwards. Uses the simulator's funded arbor coin — no network.
// ---------------------------------------------------------------------------

/// Polls sync status until the selectable XCH balance reaches `expected`,
/// draining sync events so the bounded channel never stalls the sync manager.
async fn wait_balance(app: &mut TestApp, expected: u64) -> Result<()> {
    for _ in 0..50 {
        app.drain_events();
        if app
            .get_sync_status(GetSyncStatus {})
            .await?
            .selectable_balance
            .to_u64()
            == Some(expected)
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    bail!("balance did not reach {expected} mojos");
}

/// Runs a single coin spend through the CLVM and returns its output conditions
/// (this is the "run" check). Errors propagate so callers can detect a coin
/// spend that does not even execute.
fn run_coin_spend(
    allocator: &mut Allocator,
    cs: &CoinSpendJson,
) -> Result<Vec<Condition<NodePtr>>> {
    let puzzle: Program = hex::decode(cs.puzzle_reveal.trim_start_matches("0x"))?.into();
    let solution: Program = hex::decode(cs.solution.trim_start_matches("0x"))?.into();
    let puzzle = puzzle
        .to_clvm(allocator)
        .map_err(|e| anyhow::anyhow!("to_clvm puzzle: {e}"))?;
    let solution = solution
        .to_clvm(allocator)
        .map_err(|e| anyhow::anyhow!("to_clvm solution: {e}"))?;
    let output = run_puzzle(allocator, puzzle, solution)
        .map_err(|e| anyhow::anyhow!("run_puzzle: {e}"))?;
    Vec::<Condition<NodePtr>>::from_clvm(allocator, output)
        .map_err(|e| anyhow::anyhow!("from_clvm conditions: {e}"))
}

/// Build an UNSIGNED self-send from the funded arbor coin, then RUN every coin
/// spend and check the CREATE_COIN outputs add up. The wallet only knows the
/// card's public key, so every required signature must be the card key.
#[tokio::test]
async fn test_tangem_unsigned_send_runs() -> Result<()> {
    let mut app = TestApp::new().await?;
    let (_fp, _card_pk, arbor_ph) = app.setup_tangem(1000).await?;

    wait_balance(&mut app, 1000).await?;

    let address = app.get_sync_status(GetSyncStatus {}).await?.receive_address;

    // amount + change = 1000, both back to the arbor address (self-send).
    let tx = app
        .send_xch(SendXch {
            address: address.clone(),
            amount: Amount::u64(600),
            fee: Amount::u64(0),
            memos: vec![],
            clawback: None,
            auto_submit: false,
        })
        .await?;
    assert!(!tx.coin_spends.is_empty(), "must build coin spends");

    // Only the card key can be required (observer wallet has no other keys).
    let req = app
        .required_signatures(RequiredSignatures {
            coin_spends: tx.coin_spends.clone(),
        })
        .await?;
    assert!(
        !req.signatures.is_empty(),
        "an unsigned tx must require the card signature"
    );
    for s in &req.signatures {
        assert_eq!(s.public_key, TANGEM_PUBLIC_KEY_HEX);
    }

    // RUN every coin spend; collect CREATE_COIN outputs.
    let mut allocator = Allocator::new();
    let mut outputs: Vec<(Bytes32, u64)> = Vec::new();
    for (i, cs) in tx.coin_spends.iter().enumerate() {
        let conditions = run_coin_spend(&mut allocator, cs)
            .unwrap_or_else(|e| panic!("coin spend {i} failed to run: {e}"));
        for c in conditions {
            if let Condition::CreateCoin(cc) = c {
                println!(
                    "  out coin: ph=0x{} amount={}",
                    hex::encode(cc.puzzle_hash),
                    cc.amount
                );
                outputs.push((cc.puzzle_hash, cc.amount));
            }
        }
    }

    let total: u64 = outputs.iter().map(|(_, a)| *a).sum();
    assert_eq!(total, 1000, "recipient + change must equal the spent coin");
    assert!(
        outputs.iter().any(|(ph, a)| *ph == arbor_ph && *a == 600),
        "recipient output (600) present"
    );
    assert!(
        outputs.iter().any(|(ph, a)| *ph == arbor_ph && *a == 400),
        "change output (400) present"
    );
    println!(
        "UNSIGNED SEND ok: {} coin spend(s), outputs sum {}",
        tx.coin_spends.len(),
        total
    );
    Ok(())
}

/// Build an offer with ONLY the card's public key via `make_offer_unsigned`.
/// The offer is structurally valid but, like every offer, NOT a submittable
/// standalone bundle: validating coin spend by coin spend, at least one is
/// unsatisfiable (it locks the offered coin into the settlement puzzle and
/// requires both the card signature and the absent taker's side). We confirm
/// the offered asset is sent to SETTLEMENT_PAYMENT_HASH and the card key is
/// required.
#[tokio::test]
async fn test_tangem_unsigned_offer_per_spend() -> Result<()> {
    let mut app = TestApp::new().await?;
    let (_fp, _card_pk, _arbor_ph) = app.setup_tangem(1000).await?;

    wait_balance(&mut app, 1000).await?;

    // Offer 500 mojos XCH, request 1000 mojos XCH (asset_id None == XCH).
    let offer = app
        .make_offer_unsigned(MakeOfferUnsigned {
            offered_assets: vec![OfferAmount {
                asset_id: None,
                hidden_puzzle_hash: None,
                amount: Amount::u64(500),
            }],
            requested_assets: vec![OfferAmount {
                asset_id: None,
                hidden_puzzle_hash: None,
                amount: Amount::u64(1000),
            }],
            fee: Amount::u64(0),
            receive_address: None,
            expires_at_second: None,
            coin_ids: None,
        })
        .await?;
    assert!(!offer.coin_spends.is_empty(), "offer must build coin spends");

    let req = app
        .required_signatures(RequiredSignatures {
            coin_spends: offer.coin_spends.clone(),
        })
        .await?;
    assert!(
        !req.signatures.is_empty(),
        "an unsigned offer must require the card signature"
    );
    for s in &req.signatures {
        assert_eq!(s.public_key, TANGEM_PUBLIC_KEY_HEX);
    }

    // Validate coin spend by coin spend.
    let settle: Bytes32 = SETTLEMENT_PAYMENT_HASH.into();
    let mut allocator = Allocator::new();
    let mut run_failures = 0usize;
    let mut settlement_outputs = 0usize;
    let mut aggsig_conditions = 0usize;
    let mut announcement_assertions = 0usize;
    for (i, cs) in offer.coin_spends.iter().enumerate() {
        match run_coin_spend(&mut allocator, cs) {
            Err(e) => {
                run_failures += 1;
                println!("  spend[{i}] RUN FAILED (expected for an offer): {e}");
            }
            Ok(conds) => {
                let mut to_settle = 0usize;
                for c in &conds {
                    match c {
                        Condition::CreateCoin(cc) if cc.puzzle_hash == settle => {
                            to_settle += 1;
                            settlement_outputs += 1;
                        }
                        Condition::AggSigMe(_) | Condition::AggSigUnsafe(_) => {
                            aggsig_conditions += 1
                        }
                        Condition::AssertPuzzleAnnouncement(_)
                        | Condition::AssertCoinAnnouncement(_) => announcement_assertions += 1,
                        _ => {}
                    }
                }
                println!(
                    "  spend[{i}] ran: {} condition(s), {} to settlement",
                    conds.len(),
                    to_settle
                );
            }
        }
    }

    println!(
        "OFFER built: {} coin spend(s); run_failures={run_failures}, \
         settlement_outputs={settlement_outputs}, aggsig={aggsig_conditions}, \
         announcement_assertions={announcement_assertions}",
        offer.coin_spends.len()
    );

    // The offered asset must be locked into the settlement puzzle.
    assert!(
        settlement_outputs >= 1,
        "offer must send the offered asset to SETTLEMENT_PAYMENT_HASH"
    );
    // And the offer is provably not a submittable standalone bundle: it needs
    // the card signature and/or the absent taker's announcements (or a coin
    // spend that cannot run in isolation). At least one such failure exists.
    assert!(
        run_failures + aggsig_conditions + announcement_assertions >= 1,
        "an offer cannot be a complete, submittable bundle on its own"
    );
    Ok(())
}

/// Validates the `get_offers` contract the Dart offer-listing migration
/// (`SageOffers.list`/`count`/`byOfferId`) depends on: make an offer with
/// `auto_import`, then list the store and confirm the record carries the
/// fields the mapper reads — offer string, offer_id, status and a
/// maker/taker summary.
#[tokio::test]
async fn test_get_offers_lists_made_offer() -> Result<()> {
    let mut app = TestApp::new().await?;
    let _alice = app.setup_bls(10_000_000_000_000).await?; // 10 XCH
    wait_balance(&mut app, 10_000_000_000_000).await?;

    let made = app
        .make_offer(MakeOffer {
            offered_assets: vec![OfferAmount {
                asset_id: None,
                hidden_puzzle_hash: None,
                amount: Amount::u64(1_000_000_000_000),
            }],
            requested_assets: vec![OfferAmount {
                asset_id: None,
                hidden_puzzle_hash: None,
                amount: Amount::u64(2_000_000_000_000),
            }],
            fee: Amount::u64(0),
            receive_address: None,
            expires_at_second: None,
            auto_import: true,
            coin_ids: None,
        })
        .await?;
    println!("made offer id={}", made.offer_id);

    let offers = app.get_offers(GetOffers {}).await?.offers;
    println!("get_offers returned {} record(s)", offers.len());
    assert!(!offers.is_empty(), "get_offers must return the imported offer");

    let rec = offers
        .iter()
        .find(|o| o.offer_id == made.offer_id)
        .expect("the made offer must be in the store");
    println!(
        "  status={:?} maker_assets={} taker_assets={} ts={}",
        rec.status,
        rec.summary.maker.len(),
        rec.summary.taker.len(),
        rec.creation_timestamp,
    );
    assert!(!rec.offer.is_empty(), "offer string present");
    assert!(!rec.summary.maker.is_empty(), "maker side (offered) present");
    assert!(!rec.summary.taker.is_empty(), "taker side (requested) present");
    Ok(())
}


/// Security regression: keychain secrets must be protected by the session
/// passphrase set via `unlock_keychain` / `rekey_keychain`. After re-keying
/// away from the empty (legacy) passphrase, the secrets must NOT be
/// extractable with the empty passphrase any more. See
/// SAGE_KEYCHAIN_SECURITY_PLAN.md.
#[tokio::test]
async fn test_keychain_passphrase_protects_secrets() -> Result<()> {
    let mut app = TestApp::new().await?;
    let fingerprint = app.setup_bls(0).await?;

    // Imported under the default empty passphrase (legacy behaviour): the
    // secret decrypts with no passphrase.
    let res = app.get_secret_key(GetSecretKey { fingerprint }).await?;
    assert!(res.secrets.is_some(), "secret readable under empty passphrase");

    // Re-key from empty -> a real passphrase.
    let new_pw = hex::encode(b"a-strong-derived-passphrase");
    let rekeyed = app
        .rekey_keychain(RekeyKeychain {
            old_password: String::new(),
            new_password: new_pw.clone(),
        })
        .await?;
    assert_eq!(rekeyed.rekeyed, 1, "exactly one secret re-encrypted");

    // In-memory passphrase is now the new one: the secret still decrypts.
    let res = app.get_secret_key(GetSecretKey { fingerprint }).await?;
    assert!(res.secrets.is_some(), "secret readable under new passphrase");

    // Simulate a fresh session that wrongly assumes no passphrase: extraction
    // must fail (the on-disk secret is no longer plaintext-equivalent).
    app.sage.lock().await.keychain_password = Vec::new();
    let err = app
        .get_secret_key(GetSecretKey { fingerprint })
        .await
        .err();
    assert!(
        err.is_some(),
        "empty passphrase must NOT decrypt a re-keyed secret"
    );

    // Re-unlock with the correct passphrase and confirm it works again.
    app.unlock_keychain(UnlockKeychain {
        password: new_pw,
    })
    .await?;
    let res = app.get_secret_key(GetSecretKey { fingerprint }).await?;
    assert!(res.secrets.is_some(), "secret readable after re-unlock");

    Ok(())
}
