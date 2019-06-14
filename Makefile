export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?= gluk64/franklin:server
export PROVER_DOCKER_IMAGE ?=gluk64/franklin:prover
export GETH_DOCKER_IMAGE ?= gluk64/franklin:geth
export FLATTENER_DOCKER_IMAGE ?= gluk64/franklin:flattener
export NGINX_DOCKER_IMAGE ?= gluk64/franklin-nginx:$(FRANKLIN_ENV)

# Getting started

# Check and change environment (listed here for autocomplete and documentation only)
env:	

# Get everything up and running for the first time
init:
	@bin/init

yarn:
	@cd js/franklin && yarn
	@cd js/client && yarn
	@cd js/loadtest && yarn
	@cd js/explorer && yarn
	@cd contracts && yarn


# Helpers

# This will prompt user to confirm an action on production environment
confirm_action:
	@bin/.confirm_action


# Database tools

sql = psql "$(DATABASE_URL)" -c 

db-test:
	@bin/db-test

db-test-reset:
	@bin/db-test reset

db-setup:
	@bin/db-setup

db-insert-contract:
	@bin/db-insert-contract

db-reset: confirm_action db-drop db-setup db-insert-contract
	@echo database is ready

db-migrate: confirm_action
	@cd src/storage && diesel migration run

db-drop: confirm_action
	@# this is used to clear the produciton db; cannot do `diesel database reset` because we don't own the db
	@echo DATABASE_URL=$(DATABASE_URL)
	@$(sql) 'DROP OWNED BY CURRENT_USER CASCADE' || \
		{ $(sql) 'DROP SCHEMA IF EXISTS public CASCADE' && $(sql)'CREATE SCHEMA public'; }

db-wait:
	@bin/db-wait

# Frontend clients

dist-config:
	bin/.gen_js_config > js/client/src/env-config.js
	bin/.gen_js_config > js/explorer/src/env-config.js

client: dist-config
	@cd js/client && yarn dev

explorer: dist-config
	@cd js/explorer && yarn dev

dist-client: dist-config
	@cd js/client && yarn build

dist-explorer: dist-config
	@cd js/explorer && yarn build

image-nginx: dist-client dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -f ./docker/nginx/Dockerfile .

push-image-nginx: image-nginx
	docker push "${NGINX_DOCKER_IMAGE}"

explorer-up: #dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -f ./docker/nginx/Dockerfile .
	@docker-compose up -d nginx


# Rust: cross-platform rust builder for linus

docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) ekidd/rust-musl-builder


# Rust: main stuff

prover:
	@bin/.load_keys && cargo run --release --bin prover

server:
	@cargo run --bin server

sandbox:
	@cd src/sandbox && cargo run

build-target:
	$(rust-musl-builder) sudo chown -R rust:rust /home/rust/.cargo/git /home/rust/.cargo/registry
	$(rust-musl-builder) cargo build --release

clean-target:
	$(rust-musl-builder) cargo clean

image-server: build-target
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./docker/server/Dockerfile .

image-prover: build-target
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./docker/prover/Dockerfile .

image-rust: image-server image-prover

push-image-rust: image-rust
	docker push "${SERVER_DOCKER_IMAGE}"
	docker push "${PROVER_DOCKER_IMAGE}"

# Contracts

deploy-contracts: confirm_action
	@bin/deploy-contracts

flattener = @docker run --rm -v $(shell pwd)/contracts:/home/contracts -it "${FLATTENER_DOCKER_IMAGE}"
define flatten_file
	@echo flattening $(1)
	$(flattener) -c 'solidity_flattener --output /home/contracts/flat/$(1) /home/contracts/contracts/$(1)'
endef

# Flatten contract source
flatten:
	@mkdir -p contracts/flat
	$(call flatten_file,FranklinProxy.sol)
	$(call flatten_file,Depositor.sol)
	$(call flatten_file,Exitor.sol)
	$(call flatten_file,Transactor.sol)

# Publish source to etherscan.io
source: #flatten
	@node contracts/scripts/publish-source.js
	@echo sources published

# testing
price:
	@node contracts/scripts/check-price.js

# Loadtest

run-loadtest: confirm_action
	@node js/loadtest/loadtest.js

prepare-loadtest: confirm_action
	@node js/loadtest/loadtest.js prepare

rescue: confirm_action
	@node js/loadtest/rescue.js

deposit: confirm_action
	@node contracts/scripts/deposit.js

# Devops: main

# (Re)deploy contracts and database
redeploy: confirm_action stop deploy-contracts db-reset

dev-ready = docker ps | grep -q "$(GETH_DOCKER_IMAGE)"

start-local:
	@docker ps | grep -q "$(GETH_DOCKER_IMAGE)" || { echo "Dev env not ready. Try: 'franklin dev-up'" && exit 1; }
	@docker-compose up -d --scale prover=1 server prover nginx

dockerhub-push: image-nginx image-rust
	docker push "${NGINX_DOCKER_IMAGE}"

apply-kubeconfig:
	@bin/k8s-apply

update-rust: push-image-rust apply-kubeconfig
	@kubectl patch deployment $(FRANKLIN_ENV)-server -p "{\"spec\":{\"template\":{\"metadata\":{\"labels\":{\"date\":\"$(shell date +%s)\"}}}}}"
	@kubectl patch deployment $(FRANKLIN_ENV)-prover -p "{\"spec\":{\"template\":{\"metadata\":{\"labels\":{\"date\":\"$(shell date +%s)\"}}}}}"

update-nginx: push-image-nginx apply-kubeconfig
	@kubectl patch deployment $(FRANKLIN_ENV)-nginx -p "{\"spec\":{\"template\":{\"metadata\":{\"labels\":{\"date\":\"$(shell date +%s)\"}}}}}"

update-all: update-rust update-nginx apply-kubeconfig

start-kube: apply-kubeconfig

ifeq (dev,$(FRANKLIN_ENV))
start: image-nginx image-rust start-local
else
start: apply-kubeconfig start-prover start-server start-nginx
endif

ifeq (dev,$(FRANKLIN_ENV))
stop: confirm_action
	@docker-compose stop server prover
else
stop: confirm_action stop-prover stop-server stop-nginx
endif

restart: stop start

start-prover:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-prover --replicas=1

start-nginx:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-nginx --replicas=1

start-server:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-server --replicas=1

stop-prover:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-prover --replicas=0

stop-server:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-server --replicas=0

stop-nginx:
	@bin/kube scale deployments/$(FRANKLIN_ENV)-nginx --replicas=0

# Monitoring

status:
	@curl $(API_SERVER)/api/v0.1/status; echo

log-dc:
	@docker-compose logs -f server prover

log-server:
	kubectl logs -f deployments/$(FRANKLIN_ENV)-server

log-prover:
	kubectl logs --tail 300 -f deployments/$(FRANKLIN_ENV)-prover

# Kubernetes: monitoring shortcuts

pods:
	kubectl get pods -o wide | grep -v Pending

nodes:
	kubectl get nodes -o wide


# Dev environment

dev-up:
	@{ docker ps | grep -q "$(GETH_DOCKER_IMAGE)" && echo "Dev env already running" && exit 1; } || echo -n
	@docker-compose up -d postgres geth

dev-down:
	@docker-compose stop postgres geth

geth-up: geth
	@docker-compose up geth

blockscout-migrate:
	@docker-compose up -d blockscout_postgres
	@docker-compose run blockscout /bin/sh -c "echo $MIX_ENV && mix do ecto.drop --force, ecto.create, ecto.migrate"

blockscout-up:
	@docker-compose up -d blockscout_postgres blockscout

blockscout-down:
	@docker-compose stop blockscout blockscout_postgres


# Auxillary docker containers for dev environment (usually no need to build, just use images from dockerhub)

dev-build-geth:
	@docker build -t "${GETH_DOCKER_IMAGE}" ./docker/geth

dev-build-flattener:
	@docker build -t "${FLATTENER_DOCKER_IMAGE}" ./docker/flattener

dev-push-geth:
	@docker push "${GETH_DOCKER_IMAGE}"

dev-push-flattener:
	@docker push "${FLATTENER_DOCKER_IMAGE}"

