export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?=matterlabs/server:$(IMAGE_TAG)
export PROVER_DOCKER_IMAGE ?=matterlabs/prover:$(IMAGE_TAG)
export NGINX_DOCKER_IMAGE ?= matterlabs/nginx:$(IMAGE_TAG)
export GETH_DOCKER_IMAGE ?= matterlabs/geth:latest
export CI_DOCKER_IMAGE ?= matterlabs/ci

# Getting started

# Check and change environment (listed here for autocomplete and documentation only)
env:	

# Get everything up and running for the first time
init:
	@bin/init

yarn:
	@cd js/zksync-crypto
	@cd js/zksync.js && yarn && yarn build
	@cd js/client && yarn
	@cd js/explorer && yarn
	@cd contracts && yarn
	@cd js/tests && yarn


# Helpers

# This will prompt user to confirm an action on production environment
confirm_action:
	@bin/.confirm_action

rust-checks:
	cargo fmt -- --check
	@find core/ -type f -name "*.rs" -exec touch {} +
	cargo clippy --tests --benches -- -D warnings

# Database tools

sql = psql "$(DATABASE_URL)" -c 

db-test:
	@bin/db-test.sh reset

db-test-no-reset:
	@bin/db-test.sh no-reset

db-setup:
	@bin/db-setup

db-insert-contract:
	@bin/db-insert-contract.sh

db-insert-eth-data:
	@bin/db-insert-eth-data.sh

db-reset: confirm_action db-wait db-drop db-setup db-insert-contract db-insert-eth-data
	@echo database is ready

db-migrate: confirm_action
	@cd core/storage && diesel migration run

db-drop: confirm_action
	@# this is used to clear the produciton db; cannot do `diesel database reset` because we don't own the db
	@echo DATABASE_URL=$(DATABASE_URL)
	@$(sql) 'DROP OWNED BY CURRENT_USER CASCADE' || \
		{ $(sql) 'DROP SCHEMA IF EXISTS public CASCADE' && $(sql)'CREATE SCHEMA public'; }

db-wait:
	@bin/db-wait

genesis: confirm_action
	@bin/genesis.sh

# Frontend clients

client:
	@cd js/client && yarn serve

explorer:
	@cd js/explorer && yarn serve

dist-client:
	@cd js/client && yarn build

dist-explorer:
	@cd js/explorer && yarn build

image-nginx: dist-client dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -f ./docker/nginx/Dockerfile .

push-image-nginx: image-nginx
	docker push "${NGINX_DOCKER_IMAGE}"

image-ci:
	@docker build -t "${CI_DOCKER_IMAGE}" -f ./docker/ci/Dockerfile .

push-image-ci: image-ci
	docker push "${CI_DOCKER_IMAGE}"

# Using RUST+Linux docker image (ekidd/rust-musl-builder) to build for Linux. More at https://github.com/emk/rust-musl-builder
docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry --env-file $(ZKSYNC_HOME)/etc/env/$(ZKSYNC_ENV).env
rust-musl-builder = @docker run $(docker-options) ekidd/rust-musl-builder


# Rust: main stuff


dummy-prover:
	cargo run --bin dummy_prover

prover:
	@bin/provers-launch-dev

server:
	@cargo run --bin server --release

sandbox:
	@cargo run --bin sandbox

# See more more at https://github.com/emk/rust-musl-builder#caching-builds
build-target:
	$(rust-musl-builder) sudo chown -R rust:rust /home/rust/.cargo/git /home/rust/.cargo/registry
	$(rust-musl-builder) cargo build --release

clean-target:
	$(rust-musl-builder) cargo clean

image-server: build-target
	@docker build -t "${SERVER_DOCKER_IMAGE}" -f ./docker/server/Dockerfile .

image-prover: build-target
	@docker build -t "${PROVER_DOCKER_IMAGE}" -f ./docker/prover/Dockerfile .

image-rust: image-server image-prover

push-image-server:
	docker push "${SERVER_DOCKER_IMAGE}"

push-image-prover:
	docker push "${PROVER_DOCKER_IMAGE}"

push-image-rust: image-rust
	push-image-server
	push-image-prover

# Contracts

deploy-contracts: confirm_action build-contracts
	@bin/deploy-contracts.sh

test-contracts: confirm_action build-contracts
	@bin/contracts-test.sh

build-contracts: confirm_action prepare-contracts
	@bin/prepare-test-contracts.sh
	@cd contracts && yarn build

gen-keys-if-not-present:
	test -f ${KEY_DIR}/account-${ACCOUNT_TREE_DEPTH}/VerificationKey.sol || gen-keys

prepare-contracts:
	@cp ${KEY_DIR}/account-${ACCOUNT_TREE_DEPTH}/VerificationKey.sol contracts/contracts/VerificationKey.sol || (echo "please run gen-keys" && exit 1)

# testing

ci-check:
	@ci-check.sh
	
loadtest: confirm_action
	@bin/loadtest.sh

integration-testkit: build-contracts
	@bin/integration-testkit

migration-test: build-contracts
	cargo run --bin migration_test --release

itest: # contracts simple integration tests
	@bin/prepare-test-contracts.sh
	@cd contracts && yarn itest

utest: # contracts unit tests
	@bin/prepare-test-contracts.sh
	@cd contracts && yarn unit-test

integration-simple:
	@cd js/tests && yarn && yarn simple

integration-full-exit:
	@cd js/tests && yarn && yarn full-exit

price:
	@node contracts/scripts/check-price.js

circuit-tests:
	cargo test --no-fail-fast --release -p circuit -- --ignored --test-threads 1

prover-tests:
	f cargo test -p prover --release -- --ignored

# Loadtest

run-loadtest: confirm_action
	@cd js/franklin_lib && yarn loadtest

prepare-loadtest: confirm_action
	@node js/loadtest/loadtest.js prepare

rescue: confirm_action
	@node js/loadtest/rescue.js

deposit: confirm_action
	@node contracts/scripts/deposit.js

# Devops: main

# Promote build

promote-to-stage:
	@bin/promote-to.sh stage $(ci-build)

promote-to-testnet:
	@bin/promote-to.sh testnet $(ci-build)

# (Re)deploy contracts and database
redeploy: confirm_action stop deploy-contracts db-insert-contract

init-deploy: confirm_action deploy-contracts db-insert-contract

dockerhub-push: image-nginx image-rust
	docker push "${NGINX_DOCKER_IMAGE}"

apply-kubeconfig-server:
	@bin/k8s-gen-resource-definitions
	@bin/k8s-apply-server

apply-kubeconfig-provers:
	@bin/k8s-gen-resource-definitions
	@bin/k8s-apply-provers

apply-kubeconfig-nginx:
	@bin/k8s-gen-resource-definitions
	@bin/k8s-apply-nginx

apply-kubeconfig: apply-kubeconfig-server apply-kubeconfig-provers apply-kubeconfig-nginx

update-provers: push-image-prover apply-kubeconfig-server
	@kubectl patch deployment $(ZKSYNC_ENV)-server  --namespace $(ZKSYNC_ENV) -p "{\"spec\":{\"template\":{\"metadata\":{\"labels\":{\"date\":\"$(shell date +%s)\"}}}}}"

update-server: push-image-server apply-kubeconfig-provers
	@bin/provers-patch-deployments

update-nginx: push-image-nginx apply-kubeconfig-nginx
	@kubectl patch deployment $(ZKSYNC_ENV)-nginx --namespace $(ZKSYNC_ENV) -p "{\"spec\":{\"template\":{\"metadata\":{\"labels\":{\"date\":\"$(shell date +%s)\"}}}}}"

update-all: update-rust update-nginx apply-kubeconfig

start-kube: apply-kubeconfig

ifeq (dev,$(ZKSYNC_ENV))
start: image-nginx image-rust start-local
else
start: apply-kubeconfig start-provers start-server start-nginx
endif

ifeq (dev,$(ZKSYNC_ENV))
stop:
else ifeq (ci,$(ZKSYNC_ENV))
stop:
else
stop: confirm_action stop-provers stop-server stop-nginx
endif

restart: stop start

start-provers:
	@bin/provers-scale 1

start-nginx:
	@bin/kube scale deployments/$(ZKSYNC_ENV)-nginx --namespace $(ZKSYNC_ENV) --replicas=1

start-server:
	@bin/kube scale deployments/$(ZKSYNC_ENV)-server --namespace $(ZKSYNC_ENV) --replicas=1

stop-provers:
	@bin/provers-scale 0

stop-server:
	@bin/kube scale deployments/$(ZKSYNC_ENV)-server --namespace $(ZKSYNC_ENV) --replicas=0

stop-nginx:
	@bin/kube scale deployments/$(ZKSYNC_ENV)-nginx --namespace $(ZKSYNC_ENV) --replicas=0

# Monitoring

status:
	@curl $(API_SERVER)/api/v0.1/status; echo

log-server:
	kubectl logs -f deployments/$(ZKSYNC_ENV)-server --namespace $(ZKSYNC_ENV)

log-prover:
	kubectl logs --tail 300 -f deployments/$(ZKSYNC_ENV)-prover --namespace $(ZKSYNC_ENV)

# Kubernetes: monitoring shortcuts

pods:
	kubectl get pods -o wide --namespace $(ZKSYNC_ENV) | grep -v Pending

nodes:
	kubectl get nodes -o wide


# Dev environment

dev-up:
	@docker-compose up -d postgres geth
	@docker-compose up -d tesseracts


dev-down:
	@docker-compose stop tesseracts
	@docker-compose stop postgres geth

geth-up: geth
	@docker-compose up geth


# Auxillary docker containers for dev environment (usually no need to build, just use images from dockerhub)

dev-build-geth:
	@docker build -t "${GETH_DOCKER_IMAGE}" ./docker/geth

dev-push-geth:
	@docker push "${GETH_DOCKER_IMAGE}"

# Data Restore

data-restore-setup-and-run: data-restore-build data-restore-restart

data-restore-db-prepare: confirm_action db-reset

data-restore-build:
	@cargo build -p data_restore --release --bin data_restore

data-restore-restart: confirm_action data-restore-db-prepare
	@cargo run --bin data_restore --release -- --genesis

data-restore-continue:
	@cargo run --bin data_restore --release -- --continue
