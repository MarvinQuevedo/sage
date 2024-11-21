use specta::specta;

use crate::{app_state::AppState, error::Result, state::State};

mod actions;
mod data;
mod keys;
mod offers;
mod settings;
mod transactions;

pub use actions::*;
pub use data::*;
pub use keys::*;
pub use offers::*;
pub use settings::*;
pub use transactions::*;

pub async fn initialize(state: State<AppState>) -> Result<()> {
    let state = state.lock().await;
    let mut state = state.lock().await;
    state.initialize().await
}
