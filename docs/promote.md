# Promotion of new build to target environment using Drone CI

Creat promotion job on CI to deploy to target environment.
## Promoting to staging environment (target env = stage)

1. Prepare `stage.env` file
2. Take base64 of env file:
```bash
cat $ZKSYNC_HOME/etc/env/stage.env | base64
```
3. Add/Replace CI secret `stage_env_base64`
4. Create promotion job on CI using following command where `<DRONE_CI_BUILD>` is a build number to promote to staging
```bash
zksync promote-to-stage build=<DRONE_CI_BUILD>
```

Example: 
```bash
zksync promote-to-stage ci-build=23
```

## Promoting to testnet environment (target env = testnet)

1. Prepare `testnet.env` file
2. Take base64 of env file:
```bash
cat $ZKSYNC_HOME/etc/env/stage.env | base64
```
3. Add/Replace CI secret `testnet_env_base64`
4. Create promotion job on CI using following command where `<DRONE_CI_BUILD>` is a build number to promote to staging
```bash
zksync promote-to-stage build=<DRONE_CI_BUILD>
```

Example: 
```bash
zksync promote-to-testnet ci-build=23
```
