#export FPCO_CI_REGISTRY_IMAGE ?= registry.gitlab.fpcomplete.com/chrisallen/totto
#export CI_REGISTRY_IMAGE ?= registry.gitlab.com/bitemyapp/totto
#export FPCO_DOCKER_IMAGE ?= ${FPCO_CI_REGISTRY_IMAGE}:latest
#export KUBE_SPEC = etc/kubernetes/totto.yaml
#export DOCKER_IMAGE ?= ${CI_REGISTRY_IMAGE}:latest

export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?= gluk64/franklin:server
export PROVER_DOCKER_IMAGE ?= gluk64/franklin:prover

docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) -it ekidd/rust-musl-builder

build-target:
	$(rust-musl-builder) cargo build --release

server-image: build-target
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./etc/docker/server/Dockerfile .

prover-image: build-target
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./etc/docker/prover/Dockerfile .

images: server-image prover-image

push: images
	docker push gluk64/franklin:server
	docker push gluk64/franklin:prover

up: images
	@docker-compose up -d --scale prover=1 server prover

down:
	@docker-compose stop server prover

dev-up:
	@docker-compose up -d postgres

dev-down:
	@docker-compose stop postgres
