use std::{sync::Arc, time::Duration};

use crate::application::app_service::AppService;

pub fn spawn(app_service: Arc<AppService>, stale_after: Duration, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            match app_service.mark_stale_devices_offline(stale_after).await {
                Ok(0) => {}
                Ok(count) => {
                    log::info!("marked {count} stale device(s) offline");
                }
                Err(error) => {
                    log::error!("availability watchdog failed: {error:#}");
                }
            }
        }
    });
}
