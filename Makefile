export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?=matterlabs/server:latest
export PROVER_DOCKER_IMAGE ?=matterlabs/prover:latest
export NGINX_DOCKER_IMAGE ?= matterlabs/nginx:$(FRANKLIN_ENV)

export GETH_DOCKER_IMAGE ?= gluk64/franklin:geth
export FLATTENER_DOCKER_IMAGE ?= gluk64/franklin:flattener

# Getting started

# Check and change environment (listed here for autocomplete and documentation only)
env:	

# Get everything up and running for the first time
init:
	@bin/init

yarn:
	@cd js/franklin_lib && yarn
	@cd js/client && yarn
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
	@bin/db-insert-contract.sh

update-frontend-contract:
	@bin/update-frontend-contract.sh

db-reset: confirm_action db-wait db-drop db-setup db-insert-contract update-frontend-contract
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

dist-config:
	bin/.gen_js_config > js/client/src/env-config.js
	bin/.gen_js_config > js/explorer/src/env-config.js

client:
	@cd js/client && yarn serve

explorer: dist-config
	@cd js/explorer && yarn dev

dist-client:
	@cd js/client && yarn build

dist-explorer: dist-config
	@cd js/explorer && yarn build

image-nginx: dist-client dist-explorer
	@docker build -t "${NGINX_DOCKER_IMAGE}" -f ./docker/nginx/Dockerfile .

push-image-nginx: image-nginx
	docker push "${NGINX_DOCKER_IMAGE}"

# Using RUST+Linux docker image (ekidd/rust-musl-builder) to build for Linux. More at https://github.com/emk/rust-musl-builder
docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) ekidd/rust-musl-builder


# Rust: main stuff


dummy-prover:
	cargo run --bin dummy_prover

prover:
	@cargo run --release --bin prover

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
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./docker/server/Dockerfile .

image-prover: build-target
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./docker/prover/Dockerfile .

image-rust: image-server image-prover

push-image-rust: image-rust
	docker push "${SERVER_DOCKER_IMAGE}"
	docker push "${PROVER_DOCKER_IMAGE}"

# Contracts

deploy-contracts: confirm_action
	@bin/deploy-contracts.sh

test-contracts: confirm_action build-contracts
	@bin/contracts-test.sh

build-contracts: confirm_action
	@bin/prepare-test-contracts.sh
	@cd contracts && yarn build

# Publish source to etherscan.io
publish-source:
	@node contracts/scripts/publish-source.js
	@echo sources published

# testing
price:
	@node contracts/scripts/check-price.js

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

# (Re)deploy contracts and database
ifeq (dev,$(FRANKLIN_ENV))
redeploy: confirm_action stop deploy-contracts db-insert-contract bin/minikube-copy-keys-to-host
else
redeploy: confirm_action stop deploy-contracts db-insert-contract
endif

ifeq (dev,$(FRANKLIN_ENV))
init-deploy: confirm_action deploy-contracts db-insert-contract bin/minikube-copy-keys-to-host
else
init-deploy: confirm_action deploy-contracts db-insert-contract
endif

start-local:
	@kubectl apply -f ./etc/kube/minikube/server.yaml
	@kubectl apply -f ./etc/kube/minikube/prover.yaml
	./bin/kube-update-server-vars
	@kubectl apply -f ./etc/kube/minikube/postgres.yaml
	@kubectl apply -f ./etc/kube/minikube/geth.yaml

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
	#@kubectl delete deployments --selector=app=dev-server
	#@kubectl delete deployments --selector=app=dev-prover
	#@kubectl delete deployments --selector=app=dev-nginx
	#@kubectl delete svc --selector=app=dev-server
	#@kubectl delete svc --selector=app=dev-nginx
	# not deleting postgres, geth and tesseract resources assuming they are being used for development too.
else ifeq (ci,$(FRANKLIN_ENV))
stop:
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
	@{ kubectl get po | grep -q "postgres" && echo "Dev env already running" && exit 1; } || echo -n
	@kubectl apply -f ./etc/kube/minikube/postgres.yaml
	@kubectl create configmap tesseracts-config --from-file=./etc/tesseracts/tesseracts.toml
	@kubectl apply -f ./etc/kube/minikube/geth.yaml
	./bin/update-services-url-env-vars

dev-down:
	@kubectl delete -f ./etc/kube/minikube/postgres.yaml
	@kubectl delete -f ./etc/kube/minikube/geth.yaml
	@kubectl delete configmap tesseracts-config
	./bin/reset-services-url-env-vars

# Auxillary docker containers for dev environment (usually no need to build, just use images from dockerhub)

dev-build-geth:
	@docker build -t "${GETH_DOCKER_IMAGE}" ./docker/geth

dev-build-flattener:
	@docker build -t "${FLATTENER_DOCKER_IMAGE}" ./docker/flattener

dev-push-geth:
	@docker push "${GETH_DOCKER_IMAGE}"

dev-push-flattener:
	@docker push "${FLATTENER_DOCKER_IMAGE}"
# Key generator 

make-keys:
	@cargo run -p key_generator --release --bin key_generator

 # Data Restore

data-restore-setup-and-run: data-restore-build data-restore-restart

data-restore-db-prepare: db-drop db-wait db-setup

data-restore-build:
	@cargo build -p data_restore --release --bin data_restore

data-restore-restart: confirm_action data-restore-db-prepare
	@./target/release/data_restore

data-restore-continue:
	@./target/release/data_restore
	
