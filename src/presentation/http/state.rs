use std::sync::Arc;

use crate::application::app_service::AppService;

#[derive(Clone)]
pub struct AppState {
    pub app_service: Arc<AppService>,
}
