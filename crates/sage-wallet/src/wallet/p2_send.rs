use chia::protocol::{Bytes, Bytes32, Coin, CoinSpend};
use chia_wallet_sdk::{Conditions, Memos, SpendContext};

use crate::WalletError;

use super::{p2_coin_management::fetch_first_20_coins, Wallet};

impl Wallet {
    /// Sends the given amount of XCH to the given puzzle hash, minus the fee.
    pub async fn send_xch(
        &self,
        amounts: Vec<(Bytes32, u64)>,
        fee: u64,
        memos: Vec<Bytes>,
        hardened: bool,
        reuse: bool,
        selected_coins: Option<Vec<Coin>>,
        change_puzzle_hash: Option<Bytes32>,
    ) -> Result<Vec<CoinSpend>, WalletError> {
        let total_amount = amounts
            .iter()
            .map(|(_, amount)| *amount as u128)
            .sum::<u128>()
            + fee as u128;
        let combined_amount = amounts.iter().map(|(_, amount)| amount).sum::<u64>();

        let total = combined_amount as u128 + fee as u128;

        let coins = if let Some(pre_selected) = selected_coins {
            // Validate pre-selected coins have sufficient amount
            let available: u128 = pre_selected.iter().map(|coin| coin.amount as u128).sum();
            if available < total_amount {
                return Err(WalletError::InsufficientFunds);
            }
            pre_selected
        } else {
            self.select_p2_coins(total_amount).await?
        };
        let selected: u128 = coins.iter().map(|coin| coin.amount as u128).sum();

        let change_puzzle_hash = if let Some(change_puzzle_hash) = change_puzzle_hash {
            change_puzzle_hash
        } else {
            self.p2_puzzle_hash(hardened, reuse).await?
        };

        let change: u64 = (selected - total)
            .try_into()
            .expect("change amount overflow");

        let fee_coins = if fee > 0 {
            let coins = fetch_first_20_coins(self).await?;
            let mut total_amount = 0;
            let mut required_fee_coins = vec![];
            for coin in coins {
                if total_amount >= fee as u128 {
                    break;
                }
                total_amount += coin.amount as u128;
                required_fee_coins.push(coin);
            }
            required_fee_coins
        } else {
            Vec::new()
        };

        let mut ctx = SpendContext::new();
        let mut conditions = Conditions::new();

        // Handle fee coins first
        if !fee_coins.is_empty() {
            let fee_coins_total: u128 = fee_coins.iter().map(|coin| coin.amount as u128).sum();
            let first_fee_coin: Coin = fee_coins[0].clone();
            let first_fee_coin_puzzle_hash = first_fee_coin.puzzle_hash;

            // If fee coins amount is greater than fee, create change coin for the first fee coin's puzzle hash
            if fee_coins_total > fee as u128 {
                let fee_change = (fee_coins_total - fee as u128) as u64;
                conditions = conditions.create_coin(first_fee_coin_puzzle_hash, fee_change, None);
            }

            conditions = conditions.reserve_fee(fee);
            self.spend_p2_coins(&mut ctx, fee_coins, conditions.clone())
                .await?;
            conditions = Conditions::new();
        }

        // Handle main transaction
        for (puzzle_hash, amount) in amounts {
            conditions =
                conditions.create_coin(puzzle_hash, amount, Some(Memos::new(ctx.alloc(&memos)?)));
        }

        if change > 0 {
            conditions = conditions.create_coin(change_puzzle_hash, change, None);
        }

        self.spend_p2_coins(&mut ctx, coins, conditions).await?;

        Ok(ctx.take())
    }
}

#[cfg(test)]
mod tests {
    use test_log::test;

    use crate::TestWallet;

    #[test(tokio::test)]
    async fn test_send_xch() -> anyhow::Result<()> {
        let mut test = TestWallet::new(1000).await?;

        let coin_spends = test
            .wallet
            .send_xch(
                vec![(test.puzzle_hash, 1000)],
                0,
                Vec::new(),
                false,
                true,
                None,
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.balance().await?, 1000);
        assert_eq!(test.wallet.db.spendable_coins().await?.len(), 1);

        Ok(())
    }

    #[test(tokio::test)]
    async fn test_send_xch_change() -> anyhow::Result<()> {
        let mut test = TestWallet::new(1000).await?;

        let coin_spends = test
            .wallet
            .send_xch(
                vec![(test.puzzle_hash, 250)],
                250,
                Vec::new(),
                false,
                true,
                None,
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.balance().await?, 750);
        assert_eq!(test.wallet.db.spendable_coins().await?.len(), 2);

        Ok(())
    }

    #[test(tokio::test)]
    async fn test_send_xch_hardened() -> anyhow::Result<()> {
        let mut test = TestWallet::new(1000).await?;

        let coin_spends = test
            .wallet
            .send_xch(
                vec![(test.hardened_puzzle_hash, 1000)],
                0,
                Vec::new(),
                true,
                true,
                None,
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.balance().await?, 1000);
        assert_eq!(test.wallet.db.spendable_coins().await?.len(), 1);

        let coin_spends = test
            .wallet
            .send_xch(
                vec![(test.puzzle_hash, 1000)],
                0,
                Vec::new(),
                false,
                true,
                None,
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.balance().await?, 1000);
        assert_eq!(test.wallet.db.spendable_coins().await?.len(), 1);

        Ok(())
    }
}
