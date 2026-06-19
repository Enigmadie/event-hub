use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Default)]
pub struct Metrics(Arc<MetricsInner>);

#[derive(Default)]
struct MetricsInner {
    mqtt_connected: AtomicBool,
    mqtt_connects_total: AtomicU64,
    mqtt_connection_errors_total: AtomicU64,
    mqtt_publish_messages_total: AtomicU64,
    device_events_total: AtomicU64,
    device_event_errors_total: AtomicU64,
    command_publish_success_total: AtomicU64,
    command_publish_failure_total: AtomicU64,
    http_1xx_total: AtomicU64,
    http_2xx_total: AtomicU64,
    http_3xx_total: AtomicU64,
    http_4xx_total: AtomicU64,
    http_5xx_total: AtomicU64,
    availability_watchdog: WorkerMetrics,
    scheduled_commands: WorkerMetrics,
    recurring_schedules: WorkerMetrics,
    recurring_commands: WorkerMetrics,
}

#[derive(Default)]
struct WorkerMetrics {
    success_total: AtomicU64,
    error_total: AtomicU64,
    processed_total: AtomicU64,
}

#[derive(Copy, Clone)]
pub enum Worker {
    AvailabilityWatchdog,
    ScheduledCommands,
    RecurringSchedules,
    RecurringCommands,
}

impl Worker {
    fn label(self) -> &'static str {
        match self {
            Self::AvailabilityWatchdog => "availability_watchdog",
            Self::ScheduledCommands => "scheduled_commands",
            Self::RecurringSchedules => "recurring_schedules",
            Self::RecurringCommands => "recurring_commands",
        }
    }
}

impl Metrics {
    pub fn set_mqtt_connected(&self, connected: bool) {
        self.0.mqtt_connected.store(connected, Ordering::Relaxed);
    }

    pub fn record_mqtt_connect(&self) {
        self.0.mqtt_connects_total.fetch_add(1, Ordering::Relaxed);
        self.set_mqtt_connected(true);
    }

    pub fn record_mqtt_connection_error(&self) {
        self.0
            .mqtt_connection_errors_total
            .fetch_add(1, Ordering::Relaxed);
        self.set_mqtt_connected(false);
    }

    pub fn record_mqtt_publish_message(&self) {
        self.0
            .mqtt_publish_messages_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_device_event(&self) {
        self.0.device_events_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_device_event_error(&self) {
        self.0
            .device_event_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_device_command_publish(&self, success: bool) {
        let counter = if success {
            &self.0.command_publish_success_total
        } else {
            &self.0.command_publish_failure_total
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_http_status(&self, status: axum::http::StatusCode) {
        let counter = match status.as_u16() {
            100..=199 => &self.0.http_1xx_total,
            200..=299 => &self.0.http_2xx_total,
            300..=399 => &self.0.http_3xx_total,
            400..=499 => &self.0.http_4xx_total,
            500..=599 => &self.0.http_5xx_total,
            _ => return,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_worker_success(&self, worker: Worker, processed: usize) {
        let metrics = self.worker(worker);
        metrics.success_total.fetch_add(1, Ordering::Relaxed);
        metrics
            .processed_total
            .fetch_add(processed as u64, Ordering::Relaxed);
    }

    pub fn record_worker_error(&self, worker: Worker) {
        self.worker(worker)
            .error_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn render_prometheus(&self) -> String {
        let mut out = String::new();

        push_help(
            &mut out,
            "iot_hub_mqtt_connected",
            "MQTT broker connection state.",
        );
        push_type(&mut out, "iot_hub_mqtt_connected", "gauge");
        push_metric(
            &mut out,
            "iot_hub_mqtt_connected",
            if self.0.mqtt_connected.load(Ordering::Relaxed) {
                1
            } else {
                0
            },
        );

        push_counter(
            &mut out,
            "iot_hub_mqtt_connects_total",
            "Successful MQTT broker connections.",
            self.0.mqtt_connects_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "iot_hub_mqtt_connection_errors_total",
            "MQTT connection errors seen by the event loop.",
            self.0.mqtt_connection_errors_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "iot_hub_mqtt_publish_messages_total",
            "Incoming MQTT publish packets received from Zigbee2MQTT.",
            self.0.mqtt_publish_messages_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "iot_hub_device_events_total",
            "Parsed device events accepted for processing.",
            self.0.device_events_total.load(Ordering::Relaxed),
        );
        push_counter(
            &mut out,
            "iot_hub_device_event_errors_total",
            "Device events that failed application processing.",
            self.0.device_event_errors_total.load(Ordering::Relaxed),
        );

        push_help(
            &mut out,
            "iot_hub_device_command_publish_total",
            "Device command publish attempts to Zigbee2MQTT.",
        );
        push_type(&mut out, "iot_hub_device_command_publish_total", "counter");
        push_labeled_metric(
            &mut out,
            "iot_hub_device_command_publish_total",
            &[("result", "success")],
            self.0.command_publish_success_total.load(Ordering::Relaxed),
        );
        push_labeled_metric(
            &mut out,
            "iot_hub_device_command_publish_total",
            &[("result", "failure")],
            self.0.command_publish_failure_total.load(Ordering::Relaxed),
        );

        push_help(
            &mut out,
            "iot_hub_http_requests_total",
            "HTTP responses by status class.",
        );
        push_type(&mut out, "iot_hub_http_requests_total", "counter");
        push_labeled_metric(
            &mut out,
            "iot_hub_http_requests_total",
            &[("status_class", "1xx")],
            self.0.http_1xx_total.load(Ordering::Relaxed),
        );
        push_labeled_metric(
            &mut out,
            "iot_hub_http_requests_total",
            &[("status_class", "2xx")],
            self.0.http_2xx_total.load(Ordering::Relaxed),
        );
        push_labeled_metric(
            &mut out,
            "iot_hub_http_requests_total",
            &[("status_class", "3xx")],
            self.0.http_3xx_total.load(Ordering::Relaxed),
        );
        push_labeled_metric(
            &mut out,
            "iot_hub_http_requests_total",
            &[("status_class", "4xx")],
            self.0.http_4xx_total.load(Ordering::Relaxed),
        );
        push_labeled_metric(
            &mut out,
            "iot_hub_http_requests_total",
            &[("status_class", "5xx")],
            self.0.http_5xx_total.load(Ordering::Relaxed),
        );

        push_help(
            &mut out,
            "iot_hub_worker_runs_total",
            "Background worker runs by worker and result.",
        );
        push_type(&mut out, "iot_hub_worker_runs_total", "counter");
        push_help(
            &mut out,
            "iot_hub_worker_processed_total",
            "Items processed by background workers.",
        );
        push_type(&mut out, "iot_hub_worker_processed_total", "counter");
        for worker in [
            Worker::AvailabilityWatchdog,
            Worker::ScheduledCommands,
            Worker::RecurringSchedules,
            Worker::RecurringCommands,
        ] {
            let metrics = self.worker(worker);
            push_labeled_metric(
                &mut out,
                "iot_hub_worker_runs_total",
                &[("worker", worker.label()), ("result", "success")],
                metrics.success_total.load(Ordering::Relaxed),
            );
            push_labeled_metric(
                &mut out,
                "iot_hub_worker_runs_total",
                &[("worker", worker.label()), ("result", "error")],
                metrics.error_total.load(Ordering::Relaxed),
            );
            push_labeled_metric(
                &mut out,
                "iot_hub_worker_processed_total",
                &[("worker", worker.label())],
                metrics.processed_total.load(Ordering::Relaxed),
            );
        }

        out
    }

    fn worker(&self, worker: Worker) -> &WorkerMetrics {
        match worker {
            Worker::AvailabilityWatchdog => &self.0.availability_watchdog,
            Worker::ScheduledCommands => &self.0.scheduled_commands,
            Worker::RecurringSchedules => &self.0.recurring_schedules,
            Worker::RecurringCommands => &self.0.recurring_commands,
        }
    }
}

fn push_counter(out: &mut String, name: &str, help: &str, value: u64) {
    push_help(out, name, help);
    push_type(out, name, "counter");
    push_metric(out, name, value);
}

fn push_help(out: &mut String, name: &str, help: &str) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
}

fn push_type(out: &mut String, name: &str, metric_type: &str) {
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push(' ');
    out.push_str(metric_type);
    out.push('\n');
}

fn push_metric(out: &mut String, name: &str, value: u64) {
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
}

fn push_labeled_metric(out: &mut String, name: &str, labels: &[(&str, &str)], value: u64) {
    out.push_str(name);
    out.push('{');
    for (index, (key, value)) in labels.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(value);
        out.push('"');
    }
    out.push_str("} ");
    out.push_str(&value.to_string());
    out.push('\n');
}
