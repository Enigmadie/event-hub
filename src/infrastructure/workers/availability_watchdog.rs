use std::{sync::Arc, time::Duration};

use crate::{
    application::app_service::AppService,
    observability::metrics::{Metrics, Worker},
};

pub fn spawn(
    app_service: Arc<AppService>,
    stale_after: Duration,
    interval: Duration,
    metrics: Metrics,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            match app_service.mark_stale_devices_offline(stale_after).await {
                Ok(0) => {
                    metrics.record_worker_success(Worker::AvailabilityWatchdog, 0);
                }
                Ok(count) => {
                    metrics.record_worker_success(Worker::AvailabilityWatchdog, count);
                    log::info!("marked {count} stale device(s) offline");
                }
                Err(error) => {
                    metrics.record_worker_error(Worker::AvailabilityWatchdog);
                    log::error!("availability watchdog failed: {error:#}");
                }
            }
        }
    });
}
