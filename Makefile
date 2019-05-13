#export FPCO_CI_REGISTRY_IMAGE ?= registry.gitlab.fpcomplete.com/chrisallen/totto
#export CI_REGISTRY_IMAGE ?= registry.gitlab.com/bitemyapp/totto
#export FPCO_DOCKER_IMAGE ?= ${FPCO_CI_REGISTRY_IMAGE}:latest
#export KUBE_SPEC = etc/kubernetes/totto.yaml
#export DOCKER_IMAGE ?= ${CI_REGISTRY_IMAGE}:latest

export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?= gluk64/franklin:server
export PROVER_DOCKER_IMAGE ?=gluk64/franklin:prover
export GETH_DOCKER_IMAGE ?= gluk64/franklin:geth
export FLATTENER_DOCKER_IMAGE ?= gluk64/franklin:flattener

docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) -it ekidd/rust-musl-builder
sql = psql $(DATABASE_URL) -c 

confirm_action:
	@bin/.confirm_action

# Scripts (for shell autocomplete)
env:	
	@bin/env

db-test:
	@bin/db-test

db-setup: confirm_action
	@bin/db-setup

init: dev-up env yarn db-setup redeploy

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

dist-explorer:
	@cd js/explorer && yarn build
	@bin/.gen_js_config

prover:
	@bin/.load_keys && cargo run --release --bin prover

server:
	@cargo run --bin server

sandbox:
	@cd src/sandbox && cargo run

deploy-contracts: confirm_action
	@bin/deploy-contracts

deploy-client: confirm_action
	@bin/deploy-client

db-reset: confirm_action db-drop db-setup

migrate: confirm_action
	@cd src/storage && diesel migration run

redeploy: confirm_action deploy-contracts db-reset

db-drop: confirm_action
	@# this is used to clear the produciton db; cannot do `diesel database reset` because we don't own the db
	@echo DATABASE_URL=$(DATABASE_URL)
	@$(sql) 'DROP OWNED BY CURRENT_USER CASCADE' || \
		{ $(sql) 'DROP SCHEMA IF EXISTS public CASCADE' && $(sql)'CREATE SCHEMA public'; }

build-target:
	$(rust-musl-builder) cargo build --release

clean-target:
	$(rust-musl-builder) cargo clean

server-image: build-target
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./docker/server/Dockerfile .

prover-image: build-target
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./docker/prover/Dockerfile .

images: server-image prover-image

push: images
	docker push gluk64/franklin:server
	docker push gluk64/franklin:prover

start: confirm_action images
ifeq (,$(KUBECONFIG))
	@docker-compose up -d --scale prover=1 server prover
else
	@kubectl scale deployments/server --replicas=1
	@kubectl scale deployments/prover --replicas=2
endif

stop: confirm_action
ifeq (,$(KUBECONFIG))
	@docker-compose stop server prover
else
	@kubectl scale deployments/server --replicas=0
	@kubectl scale deployments/prover --replicas=0
endif

status:
	@curl $(API_SERVER)/api/v0.1/status; echo

restart: stop start logs

log:
ifeq (,$(KUBECONFIG))
	@docker-compose logs -f server prover
else
	kubectl logs -f deployments/server
endif

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

loadtest:
	@cd js/loadtest && yarn test

# Kubernetes

kube-deploy: confirm_action
	@bin/deploy-kube

pods:
	kubectl get pods -o wide

nodes:
	kubectl get nodes -o wide

proverlogs:
	kubectl logs -f deployments/prover

dev-build-geth:
	@docker build -t "${GETH_DOCKER_IMAGE}" ./docker/geth

dev-build-flattener:
	@docker build -t "${FLATTENER_DOCKER_IMAGE}" ./docker/flattener

dev-push-geth:
	@docker push "${GETH_DOCKER_IMAGE}"

dev-push-flattener:
	@docker push "${FLATTENER_DOCKER_IMAGE}"

flattener = @docker run --rm -v $(shell pwd)/contracts:/home/contracts -it "${FLATTENER_DOCKER_IMAGE}"
define flatten_file
	@echo flattening $(1)
	$(flattener) -c 'solidity_flattener --output /home/contracts/flat/$(1) /home/contracts/contracts/$(1)'
endef

flatten:
	@mkdir -p contracts/flat
	$(call flatten_file,FranklinProxy.sol)
	$(call flatten_file,Depositor.sol)
	$(call flatten_file,Exitor.sol)
	$(call flatten_file,Transactor.sol)

source: # flatten
	node contracts/scripts/publish-source.js
