use axum::{routing::get, Router};

use super::{accounts, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/bybit/account-info", get(accounts::get_account_info))
        .with_state(state)
}
