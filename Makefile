export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?= gluk64/franklin:server
export PROVER_DOCKER_IMAGE ?=gluk64/franklin:prover
export GETH_DOCKER_IMAGE ?= gluk64/franklin:geth
export FLATTENER_DOCKER_IMAGE ?= gluk64/franklin:flattener
export NGINX_DOCKER_IMAGE ?= gluk64/franklin:nginx


# Getting started

# Get everything up and running for the first time
init: dev-up env yarn db-setup redeploy

# Check and change environment (listed here for autocomplete and documentation only)
env:	


# Helpers

# This will prompt user to confirm an action on production environment
confirm_action:
	@bin/.confirm_action


# Database tools

sql = psql $(DATABASE_URL) -c 

db-test:
	@bin/db-test

db-setup: confirm_action
	@bin/db-setup

db-reset: confirm_action db-drop db-setup

db-migrate: confirm_action
	@cd src/storage && diesel migration run

db-drop: confirm_action
	@# this is used to clear the produciton db; cannot do `diesel database reset` because we don't own the db
	@echo DATABASE_URL=$(DATABASE_URL)
	@$(sql) 'DROP OWNED BY CURRENT_USER CASCADE' || \
		{ $(sql) 'DROP SCHEMA IF EXISTS public CASCADE' && $(sql)'CREATE SCHEMA public'; }


# JS clients

yarn:
	@cd contracts && yarn
	@cd js/franklin && yarn
	@cd js/client && yarn
	@cd js/loadtest && yarn
	@cd js/explorer && yarn

client:
	@cd js/client && yarn dev

explorer:
	@cd js/explorer && yarn dev

dist-client:
	@cd js/client && yarn build
	@bin/.gen_js_config > js/client/dist/config.json

dist-explorer:
	@cd js/explorer && yarn build
	@bin/.gen_js_config > js/explorer/dist/config.json

nginx: dist-client dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -f ./docker/nginx/Dockerfile .

push-nginx: nginx
	@docker push gluk64/franklin:nginx


# Rust: cross-platform rust builder for linus

docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) -it ekidd/rust-musl-builder


# Rust: main stuff

prover:
	@bin/.load_keys && cargo run --release --bin prover

server:
	@cargo run --bin server

sandbox:
	@cd src/sandbox && cargo run

build-target:
	$(rust-musl-builder) cargo build --release

clean-target:
	$(rust-musl-builder) cargo clean

server-image: build-target
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./docker/server/Dockerfile .

prover-image: build-target
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./docker/prover/Dockerfile .

push-rust:
	docker push gluk64/franklin:server
	docker push gluk64/franklin:prover

rust-images: server-image prover-image

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


# Loadtest

loadtest:
	@cd js/loadtest && yarn test


# Devops: universal

# (Re)deploy contracts and database
redeploy: confirm_action stop deploy-contracts db-reset

# Make sure to update all images and configuration and rollout update
update: rust-images nginx push-rust push-nginx kube-deploy rollout

stop: confirm_action
ifeq (,$(KUBECONFIG))
	@docker-compose stop server prover
else
	@bin/kubectl scale deployments/server --replicas=0
	@bin/kubectl scale deployments/prover --replicas=0
endif

rollout:


# Devops: local testing

up: rust-images nginx
	@docker-compose up -d --scale prover=1 server prover nginx


# Devops: Kubernetes

# Deploy/apply kubernetes config
kube-deploy:
	@bin/deploy-kube

start: confirm_action push kube-deploy
	#@bin/kubectl scale deployments/server --replicas=1
	#@bin/kubectl scale deployments/prover --replicas=1

restart: stop start


# Kubernetes: monitoring shortcuts

pods:
	kubectl get pods -o wide

nodes:
	kubectl get nodes -o wide

proverlogs:
	kubectl logs -f deployments/prover


# Monitoring

status:
	@curl $(API_SERVER)/api/v0.1/status; echo

log:
ifeq (,$(KUBECONFIG))
	@docker-compose logs -f server prover
else
	kubectl logs -f deployments/server
endif


# Dev environment

dev-up:
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

