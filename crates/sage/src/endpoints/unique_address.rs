use chia::protocol::Bytes32;
use sage_api::{GetCatBalanceByPuzzleHashResponse, GetXchBalanceByPuzzleHashResponse};
use sage_wallet::Wallet;

use crate::{fetch_filtered_cats, fetch_filtered_coins};

pub async fn get_xch_balance_by_puzzle_hash(
    wallet: &Wallet,
    puzzle_hash: Bytes32,
) -> GetXchBalanceByPuzzleHashResponse {
    let coins = fetch_filtered_coins(wallet, None, Some(puzzle_hash))
        .await
        .unwrap();
    let balance = coins.iter().map(|coin| coin.amount).sum();
    GetXchBalanceByPuzzleHashResponse { balance }
}

pub async fn get_cat_balance_by_puzzle_hash(
    wallet: &Wallet,
    puzzle_hash: Bytes32,
    asset_id: Bytes32,
) -> GetCatBalanceByPuzzleHashResponse {
    let cats = fetch_filtered_cats(wallet, None, asset_id, Some(puzzle_hash))
        .await
        .unwrap();
    let balance = cats.iter().map(|cat| cat.coin.amount).sum();
    GetCatBalanceByPuzzleHashResponse { balance }
}
