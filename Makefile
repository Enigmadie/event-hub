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

.PHONY: help fmt check test run dev health devices device-events schedules schedule cancel-schedule turn-on turn-off clean

help:
	@printf "Available targets:\n"
	@printf "  make fmt       Format Rust code\n"
	@printf "  make check     Run cargo check\n"
	@printf "  make test      Run tests\n"
	@printf "  make run       Run the app with env defaults\n"
	@printf "  make dev       Run fmt, check, test\n"
	@printf "  make health    Call GET /health\n"
	@printf "  make devices   Call GET /devices\n"
	@printf '  make device-events GET /devices/$${DEVICE}/events\n'
	@printf '  make schedules GET /devices/$${DEVICE}/schedules\n'
	@printf '  make schedule  POST /devices/$${DEVICE}/schedules COMMAND=turn_on RUN_AT="2026-05-09 22:00:00"\n'
	@printf '  make cancel-schedule DELETE /schedules/$${SCHEDULE_ID}\n'
	@printf '  make turn-on   POST /devices/$${DEVICE}/turn-on\n'
	@printf '  make turn-off  POST /devices/$${DEVICE}/turn-off\n'
	@printf "  make clean     Remove build artifacts\n"

fmt:
	cargo fmt

check:
	cargo check

test:
	cargo test

run:
	MQTT_HOST=$(MQTT_HOST) MQTT_PORT=$(MQTT_PORT) MQTT_CLIENT_ID=$(MQTT_CLIENT_ID) HTTP_ADDR=$(HTTP_ADDR) APP_TIME_ZONE=$(APP_TIME_ZONE) DB_HOST=$(DB_HOST) DB_PORT=$(DB_PORT) DB_USERNAME=$(DB_USERNAME) DB_PASS=$(DB_PASS) DB_NAME=$(DB_NAME) DEVICE_STALE_AFTER_SECS=$(DEVICE_STALE_AFTER_SECS) DEVICE_WATCHDOG_INTERVAL_SECS=$(DEVICE_WATCHDOG_INTERVAL_SECS) SCHEDULED_COMMAND_INTERVAL_SECS=$(SCHEDULED_COMMAND_INTERVAL_SECS) SCHEDULED_COMMAND_BATCH_SIZE=$(SCHEDULED_COMMAND_BATCH_SIZE) cargo run

dev: fmt check test

health:
	curl -sS http://$(HTTP_ADDR)/health

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

turn-on:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-on DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-on

turn-off:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-off DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-off

clean:
	cargo clean
