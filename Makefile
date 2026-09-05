.DEFAULT_GOAL := help

-include .env

export

MQTT_HOST ?= 192.168.0.219
MQTT_PORT ?= 1883
MQTT_CLIENT_ID ?= event-hub
HTTP_ADDR ?= 127.0.0.1:3000
APP_TIME_ZONE ?= Europe/Moscow
DB_HOST ?= 127.0.0.1
DB_USERNAME ?= postgres
DB_PASS ?= postgres
DB_PORT ?= 5432
DB_NAME ?= event_hub
DEVICE_STALE_AFTER_SECS ?= 300
DEVICE_WATCHDOG_INTERVAL_SECS ?= 60
SCHEDULED_COMMAND_INTERVAL_SECS ?= 5
SCHEDULED_COMMAND_BATCH_SIZE ?= 25
RECURRING_SCHEDULE_INTERVAL_SECS ?= 5
RECURRING_SCHEDULE_BATCH_SIZE ?= 25
RECURRING_COMMAND_INTERVAL_SECS ?= 5
RECURRING_COMMAND_BATCH_SIZE ?= 25

.PHONY: help fmt check test run dev health metrics api-meta event-stream devices device-events schedules schedule cancel-schedule recurring-schedules recurring-schedule recurring-schedule-enable recurring-schedule-disable recurring-commands recurring-command recurring-command-enable recurring-command-disable turn-on turn-off open-cover close-cover stop-cover set-cover-position clean

help:
	@printf "Available targets:\n"
	@printf "  make fmt       Format Rust code\n"
	@printf "  make check     Run cargo check\n"
	@printf "  make test      Run tests\n"
	@printf "  make run       Run the app with env defaults\n"
	@printf "  make dev       Run fmt, check, test\n"
	@printf "  make health    Call GET /health\n"
	@printf "  make metrics   Call GET /metrics\n"
	@printf "  make api-meta  Call GET /meta\n"
	@printf "  make event-stream Watch live SSE changes\n"
	@printf "  make devices   Call GET /devices\n"
	@printf '  make device-events GET /devices/$${DEVICE}/events\n'
	@printf '  make schedules GET /devices/$${DEVICE}/schedules\n'
	@printf '  make schedule  POST /devices/$${DEVICE}/schedules COMMAND=turn_on RUN_AT="2026-05-09 22:00:00"\n'
	@printf '  make cancel-schedule DELETE /schedules/$${SCHEDULE_ID}\n'
	@printf '  make recurring-schedules GET /devices/$${DEVICE}/recurring-schedules\n'
	@printf '  make recurring-schedule POST /devices/$${DEVICE}/recurring-schedules START_TIME="10:00:00" END_TIME="00:00:00"\n'
	@printf '  make recurring-schedule-enable PATCH /recurring-schedules/$${SCHEDULE_ID} enabled=true\n'
	@printf '  make recurring-schedule-disable PATCH /recurring-schedules/$${SCHEDULE_ID} enabled=false\n'
	@printf '  make recurring-commands GET /devices/$${DEVICE}/recurring-commands\n'
	@printf '  make recurring-command POST /devices/$${DEVICE}/recurring-commands COMMAND=set_position LOCAL_TIME="09:00:00" PAYLOAD="{\\"position\\":40}"\n'
	@printf '  make recurring-command-enable PATCH /recurring-commands/$${COMMAND_ID} enabled=true\n'
	@printf '  make recurring-command-disable PATCH /recurring-commands/$${COMMAND_ID} enabled=false\n'
	@printf '  make turn-on   POST /devices/$${DEVICE}/turn-on\n'
	@printf '  make turn-off  POST /devices/$${DEVICE}/turn-off\n'
	@printf '  make open-cover POST /devices/$${DEVICE}/open\n'
	@printf '  make close-cover POST /devices/$${DEVICE}/close\n'
	@printf '  make stop-cover POST /devices/$${DEVICE}/stop\n'
	@printf '  make set-cover-position POST /devices/$${DEVICE}/position POSITION=50\n'
	@printf "  make clean     Remove build artifacts\n"

fmt:
	cargo fmt

check:
	cargo check

test:
	cargo test

run:
	MQTT_HOST=$(MQTT_HOST) MQTT_PORT=$(MQTT_PORT) MQTT_CLIENT_ID=$(MQTT_CLIENT_ID) HTTP_ADDR=$(HTTP_ADDR) APP_TIME_ZONE=$(APP_TIME_ZONE) DB_HOST=$(DB_HOST) DB_PORT=$(DB_PORT) DB_USERNAME=$(DB_USERNAME) DB_PASS=$(DB_PASS) DB_NAME=$(DB_NAME) DEVICE_STALE_AFTER_SECS=$(DEVICE_STALE_AFTER_SECS) DEVICE_WATCHDOG_INTERVAL_SECS=$(DEVICE_WATCHDOG_INTERVAL_SECS) SCHEDULED_COMMAND_INTERVAL_SECS=$(SCHEDULED_COMMAND_INTERVAL_SECS) SCHEDULED_COMMAND_BATCH_SIZE=$(SCHEDULED_COMMAND_BATCH_SIZE) RECURRING_SCHEDULE_INTERVAL_SECS=$(RECURRING_SCHEDULE_INTERVAL_SECS) RECURRING_SCHEDULE_BATCH_SIZE=$(RECURRING_SCHEDULE_BATCH_SIZE) RECURRING_COMMAND_INTERVAL_SECS=$(RECURRING_COMMAND_INTERVAL_SECS) RECURRING_COMMAND_BATCH_SIZE=$(RECURRING_COMMAND_BATCH_SIZE) cargo run

dev: fmt check test

api-meta:
	curl -sS http://$(HTTP_ADDR)/meta

event-stream:
	curl -N -sS http://$(HTTP_ADDR)/events/stream

health:
	curl -sS http://$(HTTP_ADDR)/health

metrics:
	curl -sS http://$(HTTP_ADDR)/metrics

devices:
	curl -sS http://$(HTTP_ADDR)/devices

device-events:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make device-events DEVICE=plug_plant" && exit 1)
	curl -sS "http://$(HTTP_ADDR)/devices/$(DEVICE)/events?limit=$(or $(LIMIT),50)"

schedules:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make schedules DEVICE=plug_plant" && exit 1)
	curl -sS "http://$(HTTP_ADDR)/devices/$(DEVICE)/schedules"

schedule:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make schedule DEVICE=plug_plant COMMAND=turn_on RUN_AT='2026-05-09 22:00:00'" && exit 1)
	@test -n "$(COMMAND)" || (echo "COMMAND is required: turn_on or turn_off" && exit 1)
	@test -n "$(RUN_AT)" || (echo "RUN_AT is required, example: RUN_AT='2026-05-09 22:00:00'" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/schedules -H 'content-type: application/json' -d '{"command":"$(COMMAND)","run_at":"$(RUN_AT)"}'

cancel-schedule:
	@test -n "$(SCHEDULE_ID)" || (echo "SCHEDULE_ID is required, example: make cancel-schedule SCHEDULE_ID=1" && exit 1)
	curl -sS -X DELETE http://$(HTTP_ADDR)/schedules/$(SCHEDULE_ID)

recurring-schedules:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make recurring-schedules DEVICE=plug_plant" && exit 1)
	curl -sS "http://$(HTTP_ADDR)/devices/$(DEVICE)/recurring-schedules"

recurring-schedule:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make recurring-schedule DEVICE=plug_plant START_TIME='10:00:00' END_TIME='00:00:00'" && exit 1)
	@test -n "$(START_TIME)" || (echo "START_TIME is required, example: START_TIME='10:00:00'" && exit 1)
	@test -n "$(END_TIME)" || (echo "END_TIME is required, example: END_TIME='00:00:00'" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/recurring-schedules -H 'content-type: application/json' -d '{"start_time":"$(START_TIME)","end_time":"$(END_TIME)"}'

recurring-schedule-enable:
	@test -n "$(SCHEDULE_ID)" || (echo "SCHEDULE_ID is required, example: make recurring-schedule-enable SCHEDULE_ID=1" && exit 1)
	curl -sS -X PATCH http://$(HTTP_ADDR)/recurring-schedules/$(SCHEDULE_ID) -H 'content-type: application/json' -d '{"enabled":true}'

recurring-schedule-disable:
	@test -n "$(SCHEDULE_ID)" || (echo "SCHEDULE_ID is required, example: make recurring-schedule-disable SCHEDULE_ID=1" && exit 1)
	curl -sS -X PATCH http://$(HTTP_ADDR)/recurring-schedules/$(SCHEDULE_ID) -H 'content-type: application/json' -d '{"enabled":false}'

recurring-commands:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make recurring-commands DEVICE=window_opener" && exit 1)
	curl -sS "http://$(HTTP_ADDR)/devices/$(DEVICE)/recurring-commands"

recurring-command:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make recurring-command DEVICE=window_opener COMMAND=set_position LOCAL_TIME='09:00:00' PAYLOAD='{\"position\":40}'" && exit 1)
	@test -n "$(COMMAND)" || (echo "COMMAND is required: turn_on, turn_off, open, close, stop, or set_position" && exit 1)
	@test -n "$(LOCAL_TIME)" || (echo "LOCAL_TIME is required, example: LOCAL_TIME='09:00:00'" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/recurring-commands -H 'content-type: application/json' -d '{"command":"$(COMMAND)","payload":$(or $(PAYLOAD),{}),"local_time":"$(LOCAL_TIME)"}'

recurring-command-enable:
	@test -n "$(COMMAND_ID)" || (echo "COMMAND_ID is required, example: make recurring-command-enable COMMAND_ID=1" && exit 1)
	curl -sS -X PATCH http://$(HTTP_ADDR)/recurring-commands/$(COMMAND_ID) -H 'content-type: application/json' -d '{"enabled":true}'

recurring-command-disable:
	@test -n "$(COMMAND_ID)" || (echo "COMMAND_ID is required, example: make recurring-command-disable COMMAND_ID=1" && exit 1)
	curl -sS -X PATCH http://$(HTTP_ADDR)/recurring-commands/$(COMMAND_ID) -H 'content-type: application/json' -d '{"enabled":false}'

turn-on:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-on DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-on

turn-off:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-off DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-off

open-cover:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make open-cover DEVICE=window_opener" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/open

close-cover:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make close-cover DEVICE=window_opener" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/close

stop-cover:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make stop-cover DEVICE=window_opener" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/stop

set-cover-position:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make set-cover-position DEVICE=window_opener POSITION=50" && exit 1)
	@test -n "$(POSITION)" || (echo "POSITION is required, example: POSITION=50" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/position -H 'content-type: application/json' -d '{"position":$(POSITION)}'

clean:
	cargo clean
