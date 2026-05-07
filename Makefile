.DEFAULT_GOAL := help

-include .env

export

MQTT_HOST ?= 192.168.0.219
MQTT_PORT ?= 1883
MQTT_CLIENT_ID ?= event-hub
HTTP_ADDR ?= 127.0.0.1:3000

.PHONY: help fmt check test run dev health devices turn-on turn-off clean

help:
	@printf "Available targets:\n"
	@printf "  make fmt       Format Rust code\n"
	@printf "  make check     Run cargo check\n"
	@printf "  make test      Run tests\n"
	@printf "  make run       Run the app with env defaults\n"
	@printf "  make dev       Run fmt, check, test\n"
	@printf "  make health    Call GET /health\n"
	@printf "  make devices   Call GET /devices\n"
	@printf '  make turn-on   POST /devices/$${DEVICE}/turn-on\n'
	@printf '  make turn-off  POST /devices/$${DEVICE}/turn-off\n'
	@printf "  make clean     Remove build artifacts\n"

fmt:
	cargo fmt

check:
	cargo check

test:
	cargo test

start:
	MQTT_HOST=$(MQTT_HOST) MQTT_PORT=$(MQTT_PORT) MQTT_CLIENT_ID=$(MQTT_CLIENT_ID) HTTP_ADDR=$(HTTP_ADDR) cargo run

dev: fmt check test

health:
	curl -sS http://$(HTTP_ADDR)/health

devices:
	curl -sS http://$(HTTP_ADDR)/devices

turn-on:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-on DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-on

turn-off:
	@test -n "$(DEVICE)" || (echo "DEVICE is required, example: make turn-off DEVICE=plug_plant" && exit 1)
	curl -sS -X POST http://$(HTTP_ADDR)/devices/$(DEVICE)/turn-off

clean:
	cargo clean
