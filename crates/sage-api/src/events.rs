use serde::{Deserialize, Serialize};
use specta::Type;


#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncEvent {
    Start { ip: String },
    Stop,
    Subscribed,
    Derivation,
    CoinState,
    PuzzleBatchSynced,
    CatInfo,
    DidInfo,
    NftData,
}
