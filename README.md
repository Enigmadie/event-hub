# Event Hub

Event Hub is a small IoT service for Zigbee2MQTT devices. It listens to MQTT events, stores device state and event history in Postgres, exposes a development HTTP API, and publishes commands back to Zigbee2MQTT.

## Features

- Device discovery from `zigbee2mqtt/bridge/devices`
- Current device state and availability
- Device event history
- Direct turn on/off commands
- Direct cover open/close/stop/position commands
- One-shot scheduled device commands
- Daily recurring schedules with start/end times
- Availability watchdog for stale devices
- Postgres-backed storage with startup schema bootstrap

## Configuration

Runtime configuration is loaded from `.env`.

Copy the example file and adjust it for your machine:

```bash
cp .env.example .env
```

Required/common variables:

```dotenv
MQTT_HOST=192.168.0.219
MQTT_PORT=1883
MQTT_CLIENT_ID=iot-hub
HTTP_ADDR=127.0.0.1:3000
HTTP_CORS_ALLOWED_ORIGINS=http://localhost:5173

APP_TIME_ZONE=Europe/Moscow

DB_HOST=127.0.0.1
DB_USERNAME=postgres
DB_PASS=postgres
DB_PORT=5555
DB_NAME=event_hub

DEVICE_STALE_AFTER_SECS=300
DEVICE_WATCHDOG_INTERVAL_SECS=60
SCHEDULED_COMMAND_INTERVAL_SECS=5
SCHEDULED_COMMAND_BATCH_SIZE=25
RECURRING_SCHEDULE_INTERVAL_SECS=5
RECURRING_SCHEDULE_BATCH_SIZE=25
RECURRING_COMMAND_INTERVAL_SECS=5
RECURRING_COMMAND_BATCH_SIZE=25
```

Notes:

- `APP_TIME_ZONE` is used when parsing schedule `run_at` values and formatting API timestamps.
- `HTTP_CORS_ALLOWED_ORIGINS` is a comma-separated list of dashboard origins allowed to call the API.
- Postgres timestamps are stored as `timestamptz`.
- The service creates `DB_NAME` if it does not exist, then creates/updates the required tables.
- Keep `.env` local. Update `.env.example` when adding new configuration keys.

## Local Commands

```bash
make help
make fmt
make check
make test
make dev
make run
```

API helper commands:

```bash
make health
make devices
make device-events DEVICE=plug_plant LIMIT=50
make schedules DEVICE=plug_plant
make schedule DEVICE=plug_plant COMMAND=turn_off RUN_AT='2026-05-09 22:00:00'
make cancel-schedule SCHEDULE_ID=1
make recurring-schedules DEVICE=plug_plant
make recurring-schedule DEVICE=plug_plant START_TIME='10:00:00' END_TIME='00:00:00'
make recurring-schedule-disable SCHEDULE_ID=1
make recurring-schedule-enable SCHEDULE_ID=1
make recurring-commands DEVICE=window_opener
make recurring-command DEVICE=window_opener COMMAND=set_position LOCAL_TIME='09:00:00' PAYLOAD='{"position":40}'
make recurring-command-disable COMMAND_ID=1
make recurring-command-enable COMMAND_ID=1
make turn-on DEVICE=plug_plant
make turn-off DEVICE=plug_plant
make open-cover DEVICE=window_opener
make close-cover DEVICE=window_opener
make stop-cover DEVICE=window_opener
make set-cover-position DEVICE=window_opener POSITION=50
```

## HTTP API

Default base URL:

```text
http://127.0.0.1:3000
```

### Health

```http
GET /health
```

### Metrics

```http
GET /metrics
```

Prometheus text exposition with app-native health signals:

- `iot_hub_mqtt_connected` - current MQTT broker connection state.
- `iot_hub_mqtt_connects_total` / `iot_hub_mqtt_connection_errors_total` - reconnect/error counters.
- `iot_hub_device_command_publish_total{result="success|failure"}` - command publish attempts to Zigbee2MQTT.
- `iot_hub_device_events_total` / `iot_hub_device_event_errors_total` - incoming event processing counters.
- `iot_hub_http_requests_total{status_class="..."}` - HTTP responses by status class.
- `iot_hub_worker_runs_total` / `iot_hub_worker_processed_total` - background worker status.

### List Devices

```http
GET /devices
```

Example:

```bash
curl 'http://localhost:3000/devices'
```

Response:

```json
[
  {
    "id": "plug_plant",
    "name": "plug_plant",
    "availability": "Online",
    "values": {
      "state": "ON",
      "linkquality": 236
    }
  }
]
```

### Device Events

```http
GET /devices/:id/events?limit=50
```

Example:

```bash
curl 'http://localhost:3000/devices/plug_plant/events?limit=5'
```

The URL is quoted because shells such as `zsh` treat `?` as a glob character.

Response:

```json
[
  {
    "id": 10,
    "device_id": "plug_plant",
    "kind": "AvailabilityChanged",
    "name": null,
    "state": null,
    "availability": "Offline",
    "values": null,
    "source_topic": "event-hub/watchdog",
    "payload": {
      "reason": "stale",
      "stale_after_secs": 300
    },
    "occurred_at": "2026-05-09 00:41:55"
  }
]
```

### Turn Device On

```http
POST /devices/:id/turn-on
```

Example:

```bash
curl -X POST 'http://localhost:3000/devices/plug_plant/turn-on'
```

Success status:

```text
202 Accepted
```

### Turn Device Off

```http
POST /devices/:id/turn-off
```

Example:

```bash
curl -X POST 'http://localhost:3000/devices/plug_plant/turn-off'
```

Success status:

```text
202 Accepted
```

### Control Cover Device

```http
POST /devices/:id/open
POST /devices/:id/close
POST /devices/:id/stop
POST /devices/:id/position
Content-Type: application/json
```

Position body:

```json
{
  "position": 50
}
```

Examples:

```bash
curl -X POST 'http://localhost:3000/devices/window_opener/open'
curl -X POST 'http://localhost:3000/devices/window_opener/position' \
  -H 'content-type: application/json' \
  -d '{"position":50}'
```

Success status:

```text
202 Accepted
```

### List Scheduled Commands

```http
GET /devices/:id/schedules
```

Example:

```bash
curl 'http://localhost:3000/devices/plug_plant/schedules'
```

Response:

```json
[
  {
    "id": 1,
    "device_id": "plug_plant",
    "command": "turn_off",
    "status": "pending",
    "run_at": "2026-05-09 22:00:00",
    "last_error": null
  }
]
```

### Create Scheduled Command

```http
POST /devices/:id/schedules
Content-Type: application/json
```

Body:

```json
{
  "command": "turn_off",
  "run_at": "2026-05-09 22:00:00"
}
```

Supported commands:

- `turn_on`
- `turn_off`

`run_at` is interpreted in `APP_TIME_ZONE`.

Example:

```bash
curl -X POST 'http://localhost:3000/devices/plug_plant/schedules' \
  -H 'content-type: application/json' \
  -d '{"command":"turn_off","run_at":"2026-05-09 22:00:00"}'
```

Success status:

```text
201 Created
```

### Cancel Scheduled Command

```http
DELETE /schedules/:id
```

Example:

```bash
curl -X DELETE 'http://localhost:3000/schedules/1'
```

Success status:

```text
204 No Content
```

### List Recurring Schedules

```http
GET /devices/:id/recurring-schedules
```

Example:

```bash
curl 'http://localhost:3000/devices/plug_plant/recurring-schedules'
```

Response:

```json
[
  {
    "id": 1,
    "device_id": "plug_plant",
    "start_time": "10:00:00",
    "end_time": "00:00:00",
    "enabled": true,
    "last_started_on": "2026-05-12",
    "last_ended_on": null,
    "last_error": null
  }
]
```

### Create Recurring Schedule

```http
POST /devices/:id/recurring-schedules
Content-Type: application/json
```

Body:

```json
{
  "start_time": "10:00:00",
  "end_time": "00:00:00"
}
```

The recurring schedule runs every day in `APP_TIME_ZONE`. At `start_time` it sends `turn_on`; at `end_time` it sends `turn_off`. If `end_time` is earlier than or equal to `start_time`, the off command is treated as the next local day.

Example:

```bash
curl -X POST 'http://localhost:3000/devices/plug_plant/recurring-schedules' \
  -H 'content-type: application/json' \
  -d '{"start_time":"10:00:00","end_time":"00:00:00"}'
```

Success status:

```text
201 Created
```

### Enable Or Disable Recurring Schedule

```http
PATCH /recurring-schedules/:id
Content-Type: application/json
```

Example:

```bash
curl -X PATCH 'http://localhost:3000/recurring-schedules/1' \
  -H 'content-type: application/json' \
  -d '{"enabled":false}'
```

Success status:

```text
204 No Content
```

### List Recurring Commands

```http
GET /devices/:id/recurring-commands
```

Example:

```bash
curl 'http://localhost:3000/devices/window_opener/recurring-commands'
```

Response:

```json
[
  {
    "id": 1,
    "device_id": "window_opener",
    "command": "set_position",
    "payload": {
      "position": 40
    },
    "local_time": "09:00:00",
    "enabled": true,
    "last_run_on": "2026-05-16",
    "last_error": null
  }
]
```

### Create Recurring Command

```http
POST /devices/:id/recurring-commands
Content-Type: application/json
```

Supported commands:

- `turn_on`
- `turn_off`
- `open`
- `close`
- `stop`
- `set_position`

Body:

```json
{
  "command": "set_position",
  "payload": {
    "position": 40
  },
  "local_time": "09:00:00"
}
```

`local_time` is interpreted in `APP_TIME_ZONE`. Each enabled recurring command runs at most once per local day.

Example:

```bash
curl -X POST 'http://localhost:3000/devices/window_opener/recurring-commands' \
  -H 'content-type: application/json' \
  -d '{"command":"set_position","payload":{"position":40},"local_time":"09:00:00"}'
```

Success status:

```text
201 Created
```

### Enable Or Disable Recurring Command

```http
PATCH /recurring-commands/:id
Content-Type: application/json
```

Example:

```bash
curl -X PATCH 'http://localhost:3000/recurring-commands/1' \
  -H 'content-type: application/json' \
  -d '{"enabled":false}'
```

Success status:

```text
204 No Content
```

## Runtime Behavior

### MQTT Subscriptions

The service subscribes to:

- `zigbee2mqtt/+`
- `zigbee2mqtt/+/availability`
- `zigbee2mqtt/bridge/devices`

`bridge/devices` is used for discovery only. It does not update `last_seen_at` because it is a registry snapshot, not a device heartbeat.

### Availability Watchdog

The watchdog periodically marks stale devices as `Offline`.

A device is stale when:

- `last_seen_at` is present
- `last_seen_at` is older than `DEVICE_STALE_AFTER_SECS`
- current availability is not already `Offline`

The watchdog is quiet when no devices change. It writes a device event only when a device transitions to `Offline`.

### Scheduled Commands

Scheduled commands are stored in Postgres and survive service restarts.

The worker periodically claims due jobs with `status = pending`, marks them `running`, sends the MQTT command, then marks the job `succeeded` or `failed`.

Current statuses:

- `pending`
- `running`
- `succeeded`
- `failed`
- `cancelled`

## Architecture

Project layers:

- `src/domain` - IoT domain model and rules.
- `src/application` - use cases and ports.
- `src/infrastructure` - Postgres, MQTT, Zigbee2MQTT, repositories, workers.
- `src/presentation` - HTTP API.
- `src/main.rs` - composition root.

Domain and application must not depend on concrete MQTT, Zigbee2MQTT, Axum, Postgres, or environment configuration.
