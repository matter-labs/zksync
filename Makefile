export IMAGE_TAG = $(shell git rev-parse --short HEAD)
export SERVER_DOCKER_IMAGE ?=matterlabs/server:$(IMAGE_TAG)
export SERVER_DOCKER_IMAGE_LATEST ?=matterlabs/server:latest
export PROVER_DOCKER_IMAGE ?=matterlabs/prover:$(IMAGE_TAG)
export PROVER_DOCKER_IMAGE_LATEST ?=matterlabs/prover:latest
export NGINX_DOCKER_IMAGE ?= matterlabs/nginx:$(IMAGE_TAG)
export NGINX_DOCKER_IMAGE_LATEST ?= matterlabs/nginx:latest
export GETH_DOCKER_IMAGE ?= matterlabs/geth:latest
export DEV_TICKER_DOCKER_IMAGE ?= matterlabs/dev-ticker:latest
export KEYBASE_DOCKER_IMAGE ?= matterlabs/keybase-secret:latest
export CI_DOCKER_IMAGE ?= matterlabs/ci
export FEE_SELLER_IMAGE ?=matterlabs/fee-seller:latest
export EXIT_TOOL_IMAGE ?=matterlabs/exit-tool:latest
export CI_INTEGRATION_TEST_IMAGE ?=matterlabs/ci-integration-test:latest

# Getting started

# Check and change environment (listed here for autocomplete and documentation only)
# next two target are hack that allows to pass arguments to makefile
env:	
	@bin/zkenv $(filter-out $@,$(MAKECMDGOALS))
%:
	@:

# Get everything up and running for the first time
init:
	@bin/init

yarn:
	@yarn && yarn zksync build


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

genesis: confirm_action db-reset
	@bin/genesis.sh

# Frontend clients

explorer:
	@yarn explorer serve

dist-explorer: yarn build-contracts
	@yarn explorer build

image-nginx: dist-client dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -t "${NGINX_DOCKER_IMAGE_LATEST}" -f ./docker/nginx/Dockerfile .

push-image-nginx: image-nginx
	docker push "${NGINX_DOCKER_IMAGE}"
	docker push "${NGINX_DOCKER_IMAGE_LATEST}"

image-ci:
	@docker build -t "${CI_DOCKER_IMAGE}" -f ./docker/ci/Dockerfile .

push-image-ci: image-ci
	docker push "${CI_DOCKER_IMAGE}"

image-keybase:
	@docker build -t "${KEYBASE_DOCKER_IMAGE}" -f ./docker/keybase-secrets/Dockerfile .

push-image-keybase: image-keybase
	docker push "${KEYBASE_DOCKER_IMAGE}"

image-fee-seller:
	@docker build -t "${FEE_SELLER_IMAGE}" -f ./docker/fee-seller/Dockerfile .

push-image-fee-seller: image-fee-seller
	docker push "${FEE_SELLER_IMAGE}"

# Rust: main stuff

server:
	@cargo run --bin zksync_server --release

image-server: build-contracts build-dev-contracts
	@DOCKER_BUILDKIT=1 docker build -t "${SERVER_DOCKER_IMAGE}" -t "${SERVER_DOCKER_IMAGE_LATEST}" -f ./docker/server/Dockerfile .

image-prover: build-contracts build-dev-contracts
	@DOCKER_BUILDKIT=1 docker build -t "${PROVER_DOCKER_IMAGE}" -t "${PROVER_DOCKER_IMAGE_LATEST}"  -f ./docker/prover/Dockerfile .

image-rust: image-server image-prover

push-image-server:
	docker push "${SERVER_DOCKER_IMAGE}"
	docker push "${SERVER_DOCKER_IMAGE_LATEST}"

push-image-prover:
	docker push "${PROVER_DOCKER_IMAGE}"
	docker push "${PROVER_DOCKER_IMAGE_LATEST}"

push-image-rust: image-rust push-image-server push-image-prover

# Contracts

deploy-contracts: confirm_action build-contracts
	@bin/deploy-contracts.sh

publish-contracts:
	@bin/publish-contracts.sh

test-contracts: confirm_action build-contracts
	@bin/contracts-test.sh

build-dev-contracts: confirm_action prepare-verify-contracts
	@bin/prepare-test-contracts.sh
	@yarn contracts build-dev

prepare-verify-contracts:
	@cp ${KEY_DIR}/account-${ACCOUNT_TREE_DEPTH}_balance-${BALANCE_TREE_DEPTH}/KeysWithPlonkVerifier.sol contracts/contracts/ || (echo "please download keys" && exit 1)

build-contracts: confirm_action prepare-verify-contracts
	@cargo run --release --bin gen_token_add_contract
	@yarn contracts build
	
# testing

ci-check:
	@ci-check.sh
	
integration-testkit:
	@bin/integration-testkit.sh $(filter-out $@,$(MAKECMDGOALS))

integration-test:
	@yarn ts-tests test

price:
	@node contracts/scripts/check-price.js

prover-tests:
	f cargo test -p zksync_prover --release -- --ignored

js-tests:
	@yarn zksync tests
	@yarn fee-seller tests

rust-sdk-tests:
	@bin/rust-sdk-tests.sh

# Devops: main

# Promote build

promote-to-stage:
	@bin/promote-to.sh stage $(ci-build)

promote-to-rinkeby:
    # TODO: change testnet to rinkeby with #447 issue.
	@bin/promote-to.sh testnet $(ci-build)

promote-to-ropsten:
	@bin/promote-to.sh ropsten $(ci-build)

# (Re)deploy contracts and database
redeploy: confirm_action stop init-deploy

init-deploy: confirm_action deploy-contracts db-insert-contract publish-contracts

update-images: push-image-rust push-image-nginx

update-kubeconfig:
	@bin/k8s-gen-resource-definitions
	@bin/k8s-apply

ifeq (dev,$(ZKSYNC_ENV))
start:
else
start: start-provers start-server start-nginx
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
	@bin/kube scale deployments/prover --namespace $(ZKSYNC_ENV) --replicas=1

start-nginx:
	@bin/kube scale deployments/nginx --namespace $(ZKSYNC_ENV) --replicas=1

start-server:
	@bin/kube scale deployments/server --namespace $(ZKSYNC_ENV) --replicas=1

stop-provers:
	@bin/kube scale deployments/prover --namespace $(ZKSYNC_ENV) --replicas=0

stop-server:
	@bin/kube scale deployments/server --namespace $(ZKSYNC_ENV) --replicas=0

stop-nginx:
	@bin/kube scale deployments/nginx --namespace $(ZKSYNC_ENV) --replicas=0

# Monitoring

log-server:
	kubectl logs -f deployments/server --namespace $(ZKSYNC_ENV)

log-prover:
	kubectl logs --tail 300 -f deployments/prover --namespace $(ZKSYNC_ENV)

# Kubernetes: monitoring shortcuts

pods:
	kubectl get pods -o wide --namespace $(ZKSYNC_ENV) | grep -v Pending

nodes:
	kubectl get nodes -o wide


# Dev environment

dev-up:
	@docker-compose up -d postgres geth dev-ticker
	@docker-compose up -d tesseracts

dev-down:
	@docker-compose stop tesseracts
	@docker-compose stop postgres geth dev-ticker

geth-up: geth
	@docker-compose up geth


# Auxillary docker containers for dev environment (usually no need to build, just use images from dockerhub)

dev-build-geth:
	@docker build -t "${GETH_DOCKER_IMAGE}" ./docker/geth

dev-push-geth:
	@docker push "${GETH_DOCKER_IMAGE}"

image-dev-ticker:
	@docker build -t "${DEV_TICKER_DOCKER_IMAGE}" -f ./docker/dev-ticker/Dockerfile .

push-image-dev-ticker: image-dev-ticker
	@docker push "${DEV_TICKER_DOCKER_IMAGE}"

api-test:
	@yarn ts-tests api-test

image-exit-tool:
	@docker build -t "${EXIT_TOOL_IMAGE}" -f ./docker/exit-tool/Dockerfile .

push-image-exit-tool: image-exit-tool
	@docker push "${EXIT_TOOL_IMAGE}"

image-ci-integration:
	@docker build -t "${CI_INTEGRATION_TEST_IMAGE}" ./docker/integration-test/

push-image-ci-integration: image-ci-integration
	@docker push "${CI_INTEGRATION_TEST_IMAGE}"
