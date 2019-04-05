#export FPCO_CI_REGISTRY_IMAGE ?= registry.gitlab.fpcomplete.com/chrisallen/totto
#export CI_REGISTRY_IMAGE ?= registry.gitlab.com/bitemyapp/totto
#export FPCO_DOCKER_IMAGE ?= ${FPCO_CI_REGISTRY_IMAGE}:latest
#export KUBE_SPEC = etc/kubernetes/totto.yaml
#export DOCKER_IMAGE ?= ${CI_REGISTRY_IMAGE}:latest

export CI_PIPELINE_ID ?= $(shell date +"%Y-%m-%d-%s")
export SERVER_DOCKER_IMAGE ?= server
export PROVER_DOCKER_IMAGE ?= prover

docker-options = --rm -v $(shell pwd):/home/rust/src -v cargo-git:/home/rust/.cargo/git -v cargo-registry:/home/rust/.cargo/registry
rust-musl-builder = @docker run $(docker-options) -it ekidd/rust-musl-builder

redeploy-prod:
	./bin/redeploy prod

test:
	$(rust-musl-builder) cargo --version

build:
	@cargo build

# build-test-image:
# 	docker build -t test -f ./etc/docker/test/Dockerfile .

build-server-image:
	$(rust-musl-builder) cargo build --release
	docker build -t "${SERVER_DOCKER_IMAGE}" -f ./etc/docker/server/Dockerfile .

build-prover-image:
	$(rust-musl-builder) cargo build --release
	docker build -t "${PROVER_DOCKER_IMAGE}" -f ./etc/docker/prover/Dockerfile .

build-images: build-server-image build-prover-image

prover: build-prover-image
	@docker-compose up --scale prover=1
