use chia::protocol::{Bytes32, Coin};
use chia_wallet_sdk::Cat;
use hex_literal::hex;  
use sage_wallet::Wallet;

use crate::{Error, Result};

use super::parse_coin_id;

pub const BURN_PUZZLE_HASH: [u8; 32] =
    hex!("000000000000000000000000000000000000000000000000000000000000dead");

pub async fn fetch_coins(wallet: &Wallet, coin_ids: Vec<String>) -> Result<Vec<Coin>> {
    let coin_ids = coin_ids
        .into_iter()
        .map(parse_coin_id)
        .collect::<Result<Vec<Bytes32>>>()?;

    let mut coins = Vec::new();

    for coin_id in coin_ids {
        let Some(coin_state) = wallet.db.coin_state(coin_id).await? else {
            return Err(Error::MissingCoin(coin_id));
        };

        if coin_state.spent_height.is_some() {
            return Err(Error::CoinSpent(coin_id));
        }

        coins.push(coin_state.coin);
    }

    Ok(coins)
}

pub async fn fetch_cats(wallet: &Wallet, coin_ids: Vec<String>) -> Result<Vec<Cat>> {
    let coin_ids = coin_ids
        .into_iter()
        .map(parse_coin_id)
        .collect::<Result<Vec<Bytes32>>>()?;

    let mut cats = Vec::new();

    for coin_id in coin_ids {
        let Some(coin_state) = wallet.db.coin_state(coin_id).await? else {
            return Err(Error::MissingCoin(coin_id));
        };

        if coin_state.spent_height.is_some() {
            return Err(Error::CoinSpent(coin_id));
        }

        let Some(cat) = wallet.db.cat_coin(coin_id).await? else {
            return Err(Error::MissingCatCoin(coin_id));
        };

        cats.push(cat);
    }

    Ok(cats)
}

pub async fn fetch_filtered_coins(
    wallet: &Wallet,
    selected_coins: Option<Vec<String>>,
    p2_puzzle_hash: Option<Bytes32>,
) -> Result<Vec<Coin>> {
    if let Some(coin_ids) = selected_coins {
        // If specific coins are selected, fetch and validate them
        let coins = fetch_coins(wallet, coin_ids).await?;

        // Apply puzzle hash filter if specified
        if let Some(puzzle_hash) = p2_puzzle_hash {
            Ok(coins
                .into_iter()
                .filter(|coin| coin.puzzle_hash == puzzle_hash)
                .collect())
        } else {
            Ok(coins)
        }
    } else {
        // Use existing coin fetching logic if no specific coins selected
        let mut coins = Vec::new();
        let rows = wallet.db.p2_coin_states().await?;

        for row in rows {
            if row.coin_state.spent_height.is_some() {
                continue;
            }

            if let Some(puzzle_hash) = p2_puzzle_hash {
                if row.coin_state.coin.puzzle_hash != puzzle_hash {
                    continue;
                }
            }

            coins.push(row.coin_state.coin);
        }

        Ok(coins)
    }
}

pub async fn fetch_filtered_cats(
    wallet: &Wallet,
    selected_coins: Option<Vec<String>>,
    asset_id: Bytes32,
    p2_puzzle_hash: Option<Bytes32>,
) -> Result<Vec<Cat>> {
    if let Some(coin_ids) = selected_coins {
        // If specific coins are selected, fetch and validate them
        let cats = fetch_cats(wallet, coin_ids).await?;

        // Filter by asset_id and optionally by p2_puzzle_hash
        Ok(cats
            .into_iter()
            .filter(|cat| cat.asset_id == asset_id)
            .filter(|cat| p2_puzzle_hash.map_or(true, |ph| cat.p2_puzzle_hash == ph))
            .collect())
    } else {
        // Use existing CAT fetching logic
        let mut cats = Vec::new();
        let rows = wallet.db.cat_coin_states(asset_id).await?;

        for row in rows {
            if row.coin_state.spent_height.is_some() {
                continue;
            }

            let Some(cat) = wallet.db.cat_coin(row.coin_state.coin.coin_id()).await? else {
                continue;
            };

            if let Some(puzzle_hash) = p2_puzzle_hash {
                if cat.p2_puzzle_hash != puzzle_hash {
                    continue;
                }
            }

            cats.push(cat);
        }

        Ok(cats)
    }
}
