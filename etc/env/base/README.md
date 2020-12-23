# Base configuration for zkSync stack

This folder contains the template for generating the configuration for zkSync applications. Configs in this folder are
assigned default values suitable for the development.

Since all the applications expect configuration to be set via the environment variables, these configs are compiled into
one `*.env` file, which will be loaded prior to the application launch.

Configuration files can be compiled with the `zk` subcommand:

```sh
zk config compile
```

Without any additional arguments specified, this subcommand will do the following:

1. Check whether `etc/env/current` file exists. If so, it is read and the name of the current environment is taken from
   there. Otherwise, the environment is assumed to be called `dev`.
2. Check whether the folder with the name same as current environment exists. If so, configs are read from there.
   Otherwise behavior depends on the environment name: for `dev` environment, `dev` folder will be created as a copy of
   the `base` folder. For any other environment, an error will be reported.
3. `zk` will iterate through all the `toml` files and load specified values. Once all the data is loaded, a new file
   named `<environment>.dev` is created and all the values are placed there.

It is possible to specify the config you want to compile:

```sh
zk config compile testnet # Will compile configs for the `testnet` environment.
```
