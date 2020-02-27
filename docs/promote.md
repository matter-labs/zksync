# Promotion of new build to target environment using Drone CI

Creat promotion job on CI to deploy to target environment.
## Promoting to staging environment (target env = stage)

`zksync promote-to-stage build=<DRONE_CI_BUILD>` where `<DRONE_CI_BUILD>` is a build number to promote to staging.

Example: `zksync promote-to-stage ci-build=23`
