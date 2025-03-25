use crate::Wallet;
use chia::protocol::{Bytes32, Coin};
use chia_wallet_sdk::Cat;
use sage_database::DatabaseError;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum CoinError {
    HexDecodeError(hex::FromHexError),
    InvalidCoinId(String),
    CoinNotFound(String),
    CoinAlreadySpent(String),
    CatCoinNotFound(String),
    DatabaseError(String),
}

impl fmt::Display for CoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HexDecodeError(e) => write!(f, "Hex decode error: {}", e),
            Self::InvalidCoinId(msg) => write!(f, "Invalid coin ID format: {}", msg),
            Self::CoinNotFound(id) => write!(f, "Coin not found: {}", id),
            Self::CoinAlreadySpent(id) => write!(f, "Coin already spent: {}", id),
            Self::CatCoinNotFound(id) => write!(f, "CAT coin not found: {}", id),
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl StdError for CoinError {}

impl From<DatabaseError> for CoinError {
    fn from(error: DatabaseError) -> Self {
        CoinError::DatabaseError(error.to_string())
    }
}

pub fn parse_coin_id(input: String) -> Result<Bytes32, CoinError> {
    let stripped = if let Some(stripped) = input.strip_prefix("0x") {
        stripped
    } else {
        &input
    };

    let asset_id: [u8; 32] = hex::decode(stripped)
        .map_err(CoinError::HexDecodeError)?
        .try_into()
        .map_err(|_| CoinError::InvalidCoinId(input))?;
    Ok(asset_id.into())
}

pub async fn fetch_coins(wallet: &Wallet, coin_ids: Vec<String>) -> Result<Vec<Coin>, CoinError> {
    let coin_ids = coin_ids
        .into_iter()
        .map(|id| parse_coin_id(id))
        .collect::<Result<Vec<Bytes32>, CoinError>>()?;

    let mut coins = Vec::new();

    for coin_id in &coin_ids {
        let Some(coin_state) = wallet
            .db
            .coin_state(*coin_id)
            .await
            .map_err(|e| CoinError::InvalidCoinId(e.to_string()))?
        else {
            return Err(CoinError::CoinNotFound(coin_id.to_string()));
        };

        if coin_state.spent_height.is_some() {
            return Err(CoinError::CoinAlreadySpent(coin_id.to_string()));
        }

        coins.push(coin_state.coin);
    }

    Ok(coins)
}

pub async fn fetch_cats(wallet: &Wallet, coin_ids: Vec<String>) -> Result<Vec<Cat>, CoinError> {
    let coin_ids = coin_ids
        .into_iter()
        .map(|id| parse_coin_id(id))
        .collect::<Result<Vec<Bytes32>, CoinError>>()?;

    let mut cats = Vec::new();

    for coin_id in coin_ids {
        let Some(coin_state) = wallet
            .db
            .coin_state(coin_id)
            .await
            .map_err(|e| CoinError::InvalidCoinId(e.to_string()))?
        else {
            return Err(CoinError::CatCoinNotFound(coin_id.to_string()));
        };

        if coin_state.spent_height.is_some() {
            return Err(CoinError::CoinAlreadySpent(coin_id.to_string()));
        };

        let Some(cat) = wallet
            .db
            .cat_coin(coin_id)
            .await
            .map_err(|e| CoinError::InvalidCoinId(e.to_string()))?
        else {
            return Err(CoinError::CatCoinNotFound(coin_id.to_string()));
        };

        cats.push(cat);
    }

    Ok(cats)
}

pub async fn fetch_filtered_coins(
    wallet: &Wallet,
    selected_coins: Option<Vec<String>>,
    p2_puzzle_hash: Option<Bytes32>,
) -> Result<Vec<Coin>, CoinError> {
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
        let rows = wallet.db.spendable_coins().await?;

        for coin in rows {
            if let Some(puzzle_hash) = p2_puzzle_hash {
                if coin.puzzle_hash != puzzle_hash {
                    continue;
                }
            }

            coins.push(coin);
        }

        Ok(coins)
    }
}

pub async fn fetch_filtered_cats(
    wallet: &Wallet,
    selected_coins: Option<Vec<String>>,
    asset_id: Bytes32,
    p2_puzzle_hash: Option<Bytes32>,
) -> Result<Vec<Cat>, CoinError> {
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
