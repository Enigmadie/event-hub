use std::{sync::Arc, time::Duration};

use crate::{
    application::app_service::AppService,
    observability::metrics::{Metrics, Worker},
};

pub fn spawn(app_service: Arc<AppService>, interval: Duration, batch_size: i64, metrics: Metrics) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            match app_service.run_due_scheduled_commands(batch_size).await {
                Ok(0) => {
                    metrics.record_worker_success(Worker::ScheduledCommands, 0);
                }
                Ok(count) => {
                    metrics.record_worker_success(Worker::ScheduledCommands, count);
                    log::info!("ran {count} scheduled command job(s)");
                }
                Err(error) => {
                    metrics.record_worker_error(Worker::ScheduledCommands);
                    log::error!("scheduled command worker failed: {error:#}");
                }
            }
        }
    });
}
