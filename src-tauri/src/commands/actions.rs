use chia_wallet_sdk::decode_address;
use sage_api::CatRecord;
use sage_database::{CatRow, DidRow};

use crate::{
    app_state::AppState,
    error::{Error, Result},
    state::State,
};

pub async fn remove_cat_info(state: State<AppState>, asset_id: String) -> Result<()> {
    let state = state.lock().await;
    let state = state.lock().await;
    let wallet = state.wallet()?;

    let asset_id: [u8; 32] = hex::decode(asset_id)?
        .try_into()
        .map_err(|_| Error::invalid_asset_id())?;

    wallet.db.refetch_cat(asset_id.into()).await?;

    Ok(())
}

pub async fn update_cat_info(state: State<AppState>, record: CatRecord) -> Result<()> {
    let state = state.lock().await;
    let state = state.lock().await;
    let wallet = state.wallet()?;

    let asset_id: [u8; 32] = hex::decode(record.asset_id)?
        .try_into()
        .map_err(|_| Error::invalid_asset_id())?;

    wallet
        .db
        .update_cat(CatRow {
            asset_id: asset_id.into(),
            name: record.name,
            description: record.description,
            ticker: record.ticker,
            icon: record.icon_url,
            visible: record.visible,
            fetched: true,
        })
        .await?;

    Ok(())
}

pub async fn update_did(
    state: State<AppState>,
    did_id: String,
    name: Option<String>,
    visible: bool,
) -> Result<()> {
    let state = state.lock().await;
    let state = state.lock().await;
    let wallet = state.wallet()?;

    let (launcher_id, prefix) = decode_address(&did_id)?;

    if prefix != "did:chia:" {
        return Err(Error::invalid_prefix(&prefix));
    }

    let Some(row) = wallet.db.did_row(launcher_id.into()).await? else {
        return Err(Error::invalid_launcher_id());
    };

    wallet
        .db
        .insert_did(DidRow {
            launcher_id: launcher_id.into(),
            coin_id: row.coin_id,
            name,
            is_owned: row.is_owned,
            visible,
            created_height: row.created_height,
        })
        .await?;

    Ok(())
}

pub async fn update_nft(state: State<AppState>, nft_id: String, visible: bool) -> Result<()> {
    let state = state.lock().await;
    let state = state.lock().await;
    let wallet = state.wallet()?;

    let (launcher_id, prefix) = decode_address(&nft_id)?;

    if prefix != "nft" {
        return Err(Error::invalid_prefix(&prefix));
    }

    wallet
        .db
        .set_nft_visible(launcher_id.into(), visible)
        .await?;

    Ok(())
}
