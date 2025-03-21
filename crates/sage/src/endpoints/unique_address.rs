use chia::protocol::Bytes32;
use sage_api::{GetCatBalanceByPuzzleHash, GetCatBalanceByPuzzleHashResponse, GetXchBalanceByPuzzleHash, GetXchBalanceByPuzzleHashResponse};
use sage_wallet::Wallet;

use crate::{fetch_filtered_cats, fetch_filtered_coins, parse_asset_id, parse_hash, Sage};
use crate::Error;

impl Sage {
    pub async fn get_xch_balance_by_puzzle_hash(
        &self,
        req: GetXchBalanceByPuzzleHash,
    ) -> Result<GetXchBalanceByPuzzleHashResponse, Error> {
        let wallet = self.wallet()?;
        let puzzle_hash = parse_hash(req.puzzle_hash)?;
        
        let coins = fetch_filtered_coins(&wallet, None, Some(puzzle_hash))
            .await?;
        let balance = coins.iter().map(|coin| coin.amount).sum();
        
        Ok(GetXchBalanceByPuzzleHashResponse { balance })
    }

    pub async fn get_cat_balance_by_puzzle_hash(
        &self,
        req: GetCatBalanceByPuzzleHash,
    ) -> Result<GetCatBalanceByPuzzleHashResponse, Error> {
        let wallet = self.wallet()?;
        let puzzle_hash = parse_hash(req.puzzle_hash)?;
        let asset_id = parse_asset_id(req.asset_id)?;
        
        let cats = fetch_filtered_cats(&wallet, None, asset_id, Some(puzzle_hash))
            .await?;
        let balance = cats.iter().map(|cat| cat.coin.amount).sum();
        
        Ok(GetCatBalanceByPuzzleHashResponse { balance })
    }
}
