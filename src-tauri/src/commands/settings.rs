use std::{net::IpAddr, time::Duration};

use indexmap::IndexMap; 
use sage_api::PeerRecord;
use sage_config::{Network, NetworkConfig, WalletConfig};
use sage_wallet::SyncCommand; 
use itertools::Itertools;

use crate::{app_state::AppState, error::Result, state::State};


pub async fn get_peers(state: State<AppState>) -> Result<Vec<PeerRecord>> {
    let state = state.lock().await;
    
    let state = state.lock().await;
    let peer_state = state.peer_state.lock().await;

    Ok(peer_state
        .peers_with_heights()
        .into_iter()
        .sorted_by_key(|info| info.0.socket_addr().ip())
        .map(|info| PeerRecord {
            ip_addr: info.0.socket_addr().ip().to_string(),
            port: info.0.socket_addr().port(),
            trusted: false,
            peak_height: info.1,
        })
        .collect())
}


pub async fn remove_peer(state: State<AppState>, ip_addr: IpAddr, ban: bool) -> Result<()> {
    let state = state.lock().await;
    let inner_state = state.lock().await;
    let mut peer_state = inner_state.peer_state.lock().await;

    if ban {
        peer_state.ban(ip_addr, Duration::from_secs(60 * 60), "manually banned");
    } else {
        peer_state.remove_peer(ip_addr);
    }

    Ok(())
}


pub async fn add_peer(state: State<AppState>, ip: IpAddr, trusted: bool) -> Result<()> {
    let state = state.lock().await;
    let inner_state = state.lock().await;
    
    inner_state
        .command_sender
        .send(SyncCommand::ConnectPeer { ip, trusted })
        .await?;

    Ok(())
}


pub async fn network_list(state: State< AppState>) -> Result<IndexMap<String, Network>> {
    let state = state.lock().await;
    let inner_state = state.lock().await;
    Ok(inner_state.networks.clone())
}


pub async fn network_config(state: State<AppState>) -> Result<NetworkConfig> {
    let state = state.lock().await;
    let inner_state = state.lock().await;
    Ok(inner_state.config.network.clone())
}


pub async fn set_discover_peers(state: State<AppState>, discover_peers: bool) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;
 
    if inner_state.config.network.discover_peers != discover_peers {
        inner_state.config.network.discover_peers = discover_peers;
        inner_state.save_config()?;
        inner_state
            .command_sender
            .send(SyncCommand::SetTargetPeers(if discover_peers {
                inner_state.config.network.target_peers as usize
            } else {
                0
            }))
            .await?;
    }

    Ok(())
}


pub async fn set_target_peers(state: State<AppState>, target_peers: u32) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;

    inner_state.config.network.target_peers = target_peers;
    inner_state.save_config()?;
    inner_state
        .command_sender
        .send(SyncCommand::SetTargetPeers(target_peers as usize))
        .await?;

    Ok(())
}


pub async fn set_network_id(state: State<AppState>, network_id: String) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;

    inner_state.config.network.network_id.clone_from(&network_id);
    inner_state.save_config()?;

    let network = inner_state.network();

    inner_state
        .command_sender
        .send(SyncCommand::SwitchNetwork {
            network_id,
            network: chia_wallet_sdk::Network {
                default_port: network.default_port,
                genesis_challenge: hex::decode(&network.genesis_challenge)?.try_into()?,
                dns_introducers: network.dns_introducers.clone(),
            },
        })
        .await?;

    inner_state.switch_wallet().await?;

    Ok(())
}


pub async fn wallet_config(state: State<AppState>, fingerprint: u32) -> Result<WalletConfig> {
    let state = state.lock().await;
    let inner_state = state.lock().await;
    inner_state.try_wallet_config(fingerprint).cloned()
}


pub async fn set_derive_automatically(
    state: State<AppState>,
    fingerprint: u32,
    derive_automatically: bool,
) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;

    let config = inner_state.try_wallet_config_mut(fingerprint)?;

    if config.derive_automatically != derive_automatically {
        config.derive_automatically = derive_automatically;
        inner_state.save_config()?;
    }

    Ok(())
}


pub async fn set_derivation_batch_size(
    state: State< AppState>,
    fingerprint: u32,
    derivation_batch_size: u32,
) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;

    let config = inner_state.try_wallet_config_mut(fingerprint)?;
    config.derivation_batch_size = derivation_batch_size;
    inner_state.save_config()?;

    // TODO: Update sync manager

    Ok(())
}


pub async fn rename_wallet(
    state: State< AppState>,
    fingerprint: u32,
    name: String,
) -> Result<()> {
    let state = state.lock().await;
    let mut inner_state = state.lock().await;

    let config = inner_state.try_wallet_config_mut(fingerprint)?;
    config.name = name;
    inner_state.save_config()?;

    Ok(())
}
