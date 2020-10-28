#!/bin/bash


# Example usage:
# `ci_docker_container.sh COMMAND_TO_RUN`, e.g. `ci_docker_container.sh "zksync integration-simple"`

# In this command we launch the container `matterlabs/ci-integration-test:latest`, which includes database and geth.
# `entrypoint.sh` prepares database and network for interaction, and also launches `dev-ticker-server`, `server` and `dummy-prover`.
# Note that contracts must be compiled and dummy-prover should be enabled prior to the command launch, as we mount $ZKSYNC_HOME from
# the host system inside of the container, and expect environment to be prepared for the launch.
docker run  -v $ZKSYNC_HOME:/usr/src/zksync matterlabs/ci-integration-test:latest bash -c "/usr/local/bin/entrypoint.sh && $1"
