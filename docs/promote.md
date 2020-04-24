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
zksync promote-to-stage ci-build=<DRONE_CI_BUILD>
```

Example: 
```bash
zksync promote-to-stage ci-build=23
```

## Promoting to testnet environment (target env = testnet or ropsten)

// TODO: change testnet to rinkeby with issue #447.
Process of promoting a build to testnet(rinkeby) and ropsten is the same. 
1. Prepare env file, either `testnet.env` or `ropsten.env`
2. Take base64 of env file (note: some base64 commands add `\n`, you should remove them with `--wrap=0` option):
```bash
cat $ZKSYNC_HOME/etc/env/your_file.env | base64
```
3. Add/Replace CI secret, either `testnet_env_base64`(rinkeby) or `ropsten_env_base64`. 
4. Create promotion job on CI using following command where `<DRONE_CI_BUILD>` is a build number to promote to staging
```bash
zksync promote-to-rinkeby ci-build=<DRONE_CI_BUILD>
OR
zksync promote-to-ropsten ci-build=<DRONE_CI_BUILD>
```

Example: 
```bash
zksync promote-to-rinkeby ci-build=23
```
