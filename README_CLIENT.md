# launch client:

```sh
# new terminal
# cd $FRANKLIN_HOME
. bin/.setup_env
franklin init
export CONTRACT_ADDR=0xE4F7bB162959eF6d0375Cbd7f928012b8e9873cb
franklin server

# new terminal
# cd $FRANKLIN_HOME
. bin/.setup_env
make dist-config
cd js/client
export CONTRACT_ADDR=0xE4F7bB162959eF6d0375Cbd7f928012b8e9873cb
yarn webpack-dev-server --port 9000 --open --hot --define process.env.NODE_ENV='"development"' --define process.env.CONTRACT_ADDR="'$CONTRACT_ADDR'"
```
