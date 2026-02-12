use crate::application::accounts::AccountService;

#[derive(Clone)]
pub struct AppState {
    pub account_service: AccountService,
}
