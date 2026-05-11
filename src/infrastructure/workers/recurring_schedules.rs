use std::{sync::Arc, time::Duration};

use crate::application::app_service::AppService;

pub fn spawn(app_service: Arc<AppService>, interval: Duration, batch_size: i64) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            match app_service.run_due_recurring_schedules(batch_size).await {
                Ok(0) => {}
                Ok(count) => {
                    log::info!("ran {count} recurring schedule command(s)");
                }
                Err(error) => {
                    log::error!("recurring schedule worker failed: {error:#}");
                }
            }
        }
    });
}
