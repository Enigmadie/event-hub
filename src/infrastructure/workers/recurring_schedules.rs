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

            match app_service.run_due_recurring_schedules(batch_size).await {
                Ok(0) => {
                    metrics.record_worker_success(Worker::RecurringSchedules, 0);
                }
                Ok(count) => {
                    metrics.record_worker_success(Worker::RecurringSchedules, count);
                    log::info!("ran {count} recurring schedule command(s)");
                }
                Err(error) => {
                    metrics.record_worker_error(Worker::RecurringSchedules);
                    log::error!("recurring schedule worker failed: {error:#}");
                }
            }
        }
    });
}
