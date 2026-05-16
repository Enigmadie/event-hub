use std::{sync::Arc, time::Duration};

use crate::application::app_service::AppService;

pub fn spawn(app_service: Arc<AppService>, interval: Duration, batch_size: i64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            match app_service.run_due_recurring_commands(batch_size).await {
                Ok(0) => {}
                Ok(count) => {
                    log::info!("ran {count} recurring device command(s)");
                }
                Err(error) => {
                    log::error!("recurring device command worker failed: {error:#}");
                }
            }
        }
    });
}
