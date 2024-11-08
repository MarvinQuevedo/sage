use chia::protocol::{Bytes32, CoinSpend, Program};
use chia_wallet_sdk::{Conditions, Did, HashedPtr, Launcher, SpendContext, StandardLayer};

use crate::WalletError;

use super::Wallet;

impl Wallet {
    pub async fn create_did(
        &self,
        fee: u64,
        hardened: bool,
        reuse: bool,
    ) -> Result<(Vec<CoinSpend>, Did<()>), WalletError> {
        let total_amount = fee as u128 + 1;
        let coins = self.select_p2_coins(total_amount).await?;
        let selected: u128 = coins.iter().map(|coin| coin.amount as u128).sum();

        let change: u64 = (selected - total_amount)
            .try_into()
            .expect("change amount overflow");

        let p2_puzzle_hash = self.p2_puzzle_hash(hardened, reuse).await?;

        let mut ctx = SpendContext::new();

        let synthetic_key = self.db.synthetic_key(coins[0].puzzle_hash).await?;
        let p2 = StandardLayer::new(synthetic_key);
        let (mut conditions, did) =
            Launcher::new(coins[0].coin_id(), 1).create_simple_did(&mut ctx, &p2)?;

        if fee > 0 {
            conditions = conditions.reserve_fee(fee);
        }

        if change > 0 {
            conditions = conditions.create_coin(p2_puzzle_hash, change, Vec::new());
        }

        self.spend_p2_coins(&mut ctx, coins, conditions).await?;

        Ok((ctx.take(), did))
    }

    pub async fn transfer_did(
        &self,
        did_id: Bytes32,
        puzzle_hash: Bytes32,
        fee: u64,
        hardened: bool,
        reuse: bool,
    ) -> Result<(Vec<CoinSpend>, Did<Program>), WalletError> {
        let Some(did) = self.db.spendable_did(did_id).await? else {
            return Err(WalletError::MissingDid(did_id));
        };

        let total_amount = fee as u128;
        let coins = self.select_p2_coins(total_amount).await?;
        let selected: u128 = coins.iter().map(|coin| coin.amount as u128).sum();

        let change: u64 = (selected - total_amount)
            .try_into()
            .expect("change amount overflow");

        let p2_puzzle_hash = self.p2_puzzle_hash(hardened, reuse).await?;

        let mut ctx = SpendContext::new();

        let did_metadata_ptr = ctx.alloc(&did.info.metadata)?;
        let did = did.with_metadata(HashedPtr::from_ptr(&ctx.allocator, did_metadata_ptr));

        let synthetic_key = self.db.synthetic_key(did.info.p2_puzzle_hash).await?;
        let p2 = StandardLayer::new(synthetic_key);

        let new_did = did.transfer(&mut ctx, &p2, puzzle_hash, Conditions::new())?;

        let mut conditions = Conditions::new().assert_concurrent_spend(did.coin.coin_id());

        if fee > 0 {
            conditions = conditions.reserve_fee(fee);
        }

        if change > 0 {
            conditions = conditions.create_coin(p2_puzzle_hash, change, Vec::new());
        }

        self.spend_p2_coins(&mut ctx, coins, conditions).await?;

        let new_did = new_did.with_metadata(ctx.serialize(&new_did.info.metadata)?);

        Ok((ctx.take(), new_did))
    }
}