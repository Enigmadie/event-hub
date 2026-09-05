# HTTP client integration

These extensions are shared by every HTTP client (web/mobile applications, Telegram bots,
CLI tools, and other integrations). They contain no UI labels or client-specific state.
Existing command URLs, successful response shapes/fields, and HTTP status codes remain compatible.
`GET /devices` gains an additive `supported_commands` field. Clients should tolerate unknown fields.
Error responses now contain JSON. `/health` and `/metrics` retain their operational contracts.

## Hub metadata

`GET /meta`

```json
{
  "api_version": 1,
  "time_zone": "Europe/Moscow",
  "timestamp_format": "YYYY-MM-DD HH:mm:ss",
  "schedule_time_basis": "hub_local",
  "event_stream": {
    "path": "/events/stream",
    "replay": false,
    "scope": "instance"
  }
}
```

`time_zone` is the configured `APP_TIME_ZONE`. One-time `run_at` input, returned timestamps,
and daily schedule times use this zone. Daily schedules are wall-clock rules in the hub's
zone, not the phone's/browser's zone. Existing timestamp strings intentionally keep their
format without an offset. Do not pass them to `new Date()` and implicitly interpret them
in the client's local zone. Use `time_zone` when converting them to absolute instants.
Language selection belongs to the client and does not change scheduling semantics.

## Device commands

`GET /devices` retains `id`, `name`, `availability`, and `values`, and adds:

```json
{
  "id": "plug_plant",
  "name": "plug_plant",
  "availability": "Online",
  "values": { "state": "ON" },
  "supported_commands": ["turn_on", "turn_off"]
}
```

Values are stable command identifiers: `turn_on`, `turn_off`, `open`, `close`, `stop`,
`set_position`. Their labels are translated by clients. Unknown future identifiers should
be ignored by clients that do not implement them.

- `null`: discovery information is unavailable; this does **not** mean all commands are supported.
- `[]`: discovery information is available, but none of the hub's current command payloads
  are supported (for example, a read-only sensor).
- A nonempty array lists payloads the current gateway supports for that device.

Capabilities are extracted in the Zigbee2MQTT adapter from
[`definition.exposes`](https://www.zigbee2mqtt.io/guide/usage/exposes.html), including writable
access and accepted values. Nested light/switch/cover/fan features are supported. Endpoint-specific
and composite payloads are deliberately not advertised because the existing gateway cannot
address them. A writable `position` must support the current 0–100 integer range.
This is a subset of all Zigbee2MQTT features, not a complete device capability model.

Capabilities are persisted in the new nullable `devices.supported_commands` JSONB column.
The startup migration adds it without replacing existing rows. Existing devices report `null`
until their next discovery message; the retained `zigbee2mqtt/bridge/devices` message normally
supplies it on MQTT subscription/reconnect. State/measurement reports preserve capabilities.

This field is descriptive: existing command handlers retain their behavior for compatibility.
It is not authorization, delivery confirmation, or a new restriction on bot commands.
One-time schedules still accept only `turn_on`/`turn_off`; recurring commands accept the six
identifiers above. Device command support does not expand an endpoint's scheduling contract.

## Error responses

```json
{
  "error": {
    "code": "invalid_position",
    "message": "Position must be an integer from 0 to 100.",
    "field": "position"
  }
}
```

`code` is the machine-readable identifier; `message` is an English diagnostic fallback,
not a localized interface string. `field` is optional. Clients can translate by code,
highlight the field, and fall back to a generic error for unknown codes. Internal database
and gateway diagnostics are logged on the server rather than returned to clients.

| Code | HTTP status | Meaning |
| --- | --- | --- |
| `invalid_command` | 400 | Command identifier is not accepted by the endpoint |
| `invalid_position` | 400 | Position value/payload is outside the accepted range |
| `invalid_request` | 400 / 422 | Invalid JSON, path/query/body, or rejected schedule input |
| `not_found` | 404 | Unknown route/resource |
| `method_not_allowed` | 405 | Unsupported HTTP method |
| `payload_too_large` | 413 | Request exceeds the body limit |
| `unsupported_media_type` | 415 | JSON content type is required |
| `command_delivery_failed` | 502 | Command gateway could not accept the publish |
| `service_unavailable` | 503 | Service unavailable (health probe keeps its own format) |
| `internal_error` | 500 | Server-side failure |

Axum extractor errors also use this envelope. Missing/incorrectly typed position fields
can therefore produce `invalid_request` before command-specific validation runs.
Existing schedule creation errors retain their 400 status; the API does not yet distinguish
all repository failure causes. Existing empty-list and no-op update semantics are unchanged.

## Live changes (SSE)

`GET /events/stream`, optionally `?device_id=plug_plant`.

```text
event: resync
data: {"reason":"connected"}

event: change
data: {"kind":"devices_changed","device_id":"plug_plant"}
```

Supported change kinds:

- `devices_changed`: discovery, state, measurements, availability, or watchdog updates.
- `schedules_changed`: schedule creation/cancellation/toggling or worker execution updates.
- `command_accepted`: the gateway accepted a command publish. This is **not** acknowledgement
  that the physical device executed it. Wait for device state reports for actual state.

Changes are published from application use cases, so HTTP requests, direct bot integrations
using the same service, MQTT reports, and workers follow the same notification path.
When embedding `AppService` outside the standard binary, attach a `ChangePublisher` with
`with_change_publisher`; the original constructor defaults to a no-op publisher.

This stream is **live resource invalidation**, not a durable event delivery queue:

1. Open the stream before fetching initial state. On every `resync`, fetch current state via
   `GET /devices` and the relevant event/schedule lists.
2. On a `change`, refetch the affected resources. Coalesce bursts and account for changes
   arriving during an in-flight fetch, so the final fetch reflects the newest notification.
3. `device_id: null` means a global invalidation and reaches all device-filtered subscribers.
4. Every connection starts with `resync` (`connected`). A slow subscriber receives `resync`
   (`lagged`) if it falls behind the bounded 256-message buffer. Publishers never wait for it.
5. The server sends keepalive comments every 15 seconds. Browser `EventSource` reconnects;
   other clients should reconnect with backoff. `Last-Event-ID` is not a replay cursor.
6. Notifications cover one running hub instance. Do not assume cross-instance delivery or
   exactly-once processing. Durable bot workflows should use persisted state/history; a
   shared broker/outbox and replay contract would be a separate feature.

A read-only debugging example:

```sh
curl -N 'http://127.0.0.1:3000/events/stream?device_id=plug_plant'
```

## Access from a phone

No iPhone system changes are required. Configure deployment:

- Set `PUBLIC_IOT_API_BASE_URL` in the dashboard build to a URL reachable from the phone.
  `localhost` on the phone refers to the phone itself.
- If the API and dashboard use different origins, add the **dashboard origin** to
  `HTTP_CORS_ALLOWED_ORIGINS` (comma-separated). CORS is browser access policy, not authentication.
- Ensure the API listens on an interface reachable from the dashboard, or place both behind
  a reverse proxy. HTTPS deployment should expose an HTTPS API too. Disable buffering for
  `/events/stream` in the proxy; the response includes `X-Accel-Buffering: no`.

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
TEST_DATABASE_URL=postgres://user@127.0.0.1:5432/test_db cargo test --test postgres_api -- --ignored
```

The Postgres test requires a disposable test database. It creates a unique schema, verifies
idempotent migrations, persisted capabilities, legacy JSON fields, schedules, and notifications,
then removes its schema. HTTP tests use a stub command gateway and never send MQTT commands.
