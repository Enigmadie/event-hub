use super::state::AppState;
use crate::application::changes::HubChange;
use axum::{
    extract::{Query, State},
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
};
use futures_util::{Stream, stream};
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};
use tokio::sync::broadcast;

#[derive(Deserialize)]
pub struct EventQuery {
    device_id: Option<String>,
}

pub async fn events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> impl IntoResponse {
    let stream = change_stream(state.changes.subscribe(), query.device_id);
    (
        [("cache-control", "no-cache"), ("x-accel-buffering", "no")],
        Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    )
}

fn resync(reason: &'static str) -> Event {
    Event::default()
        .event("resync")
        .data(format!("{{\"reason\":\"{reason}\"}}"))
}

fn change_stream(
    receiver: broadcast::Receiver<HubChange>,
    device_id: Option<String>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream::unfold(
        (receiver, device_id, true),
        |(mut receiver, device_id, first)| async move {
            if first {
                return Some((Ok(resync("connected")), (receiver, device_id, false)));
            }
            loop {
                let event = match receiver.recv().await {
                    Ok(change) => {
                        if device_id.as_ref().is_some_and(|id| {
                            change
                                .device_id
                                .as_ref()
                                .is_some_and(|changed| changed != id)
                        }) {
                            continue;
                        }
                        // Serialization of these string/enum-only fields cannot fail.
                        Event::default()
                            .event("change")
                            .json_data(change)
                            .expect("serializable hub change")
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => resync("lagged"),
                    Err(broadcast::error::RecvError::Closed) => return None,
                };
                return Some((Ok(event), (receiver, device_id, false)));
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::changes::ChangeKind;
    use futures_util::{StreamExt, pin_mut};
    #[tokio::test]
    async fn starts_with_resync_filters_devices_and_ends_on_close() {
        let (sender, receiver) = broadcast::channel(2);
        let stream = change_stream(receiver, Some("plug".into()));
        pin_mut!(stream);
        assert!(stream.next().await.unwrap().is_ok());
        sender
            .send(HubChange {
                kind: ChangeKind::DevicesChanged,
                device_id: Some("other".into()),
            })
            .unwrap();
        sender
            .send(HubChange {
                kind: ChangeKind::SchedulesChanged,
                device_id: None,
            })
            .unwrap();
        assert!(stream.next().await.unwrap().is_ok());
        drop(sender);
        assert!(stream.next().await.is_none());
    }
    #[tokio::test]
    async fn a_slow_subscriber_can_resync_without_blocking_publishers() {
        let (sender, receiver) = broadcast::channel(1);
        let stream = change_stream(receiver, None);
        pin_mut!(stream);
        let _ = stream.next().await.unwrap().unwrap();
        for _ in 0..10 {
            sender
                .send(HubChange {
                    kind: ChangeKind::DevicesChanged,
                    device_id: None,
                })
                .unwrap();
        }
        // A resync event, followed by the remaining change, then EOF.
        assert!(stream.next().await.unwrap().is_ok());
        assert!(stream.next().await.unwrap().is_ok());
        drop(sender);
        assert!(stream.next().await.is_none());
    }
}
