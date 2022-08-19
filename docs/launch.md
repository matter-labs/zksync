# Running the application

This document covers common scenarios of launching zkSync applications set locally.

## Prerequisites

Prepare dev environment prerequisites; see [Installing dependencies](./setup-dev.md)

## Setup local dev environment

Setup:

```
zk # installs and builds zk itself
zk init
```

During the first initialization you have to download around 8 GB of setup files, this should only have to take place
once. If you have a problem on this step of the initialization, see help for the `zk run plonk-setup` command.

If you face any other problems with the `zk init` command, go to the [Troubleshooting](##Troubleshooting) section at the
end of this file. There are solutions for some common error cases.

To completely reset the dev environment:

- Stop services:

  ```
  zk down
  ```

- Repeat the setup procedure above

If `zk init` has already been executed, and now you only need to start docker containers (e.g. after reboot), simply
launch:

```
zk up
```

## Add certificates

- Install [mkcert](https://github.com/FiloSottile/mkcert)
  - `brew install mkcert`
- Run: `mkcert --install`
- Run: `mkcert 127.0.0.1 localhost`
- Rename `127.0.0.1+1-key.pem` to `key.pem`
- Rename `127.0.0.1+1.pem` to `cert.pem`
- Put both files in the root directory
- Set environment variables for node.js as
  [documented here](https://github.com/FiloSottile/mkcert#using-the-root-with-nodejs)
  - Ex: export NODE_EXTRA_CA_CERTS="$(mkcert -CAROOT)/rootCA.pem"

## (Re)deploy db and contraсts

```
zk contract redeploy
```

## Environment configurations

Env config files are held in `etc/env/`.

List configurations:

```
zk env
```

Switch between configurations:

```
zk env <ENV_NAME>
```

Default confiruration is `dev.env`, which is generated automatically
from `dev.env.example` during `zk init` commandexecution.

## Build and run server + prover locally for development

Run server:

```
zk server
```

The server is configured using env files in `./etc/env` directory. After the first initialization, the
`./etc/env/dev.env` file will be created. By default, this file is copied from the `./etc/env/dev.env.example` template.

The server can produce blocks of different sizes; the list of available sizes is determined by the
`SUPPORTED_BLOCK_CHUNKS_SIZES` environment variable. Block sizes which will actually be produced by the server can be
configured using the `BLOCK_CHUNK_SIZES` environment variable.

Note: proof generation for large blocks requires a lot of resources and an average user machine is only capable
ofcreating proofs for the smallest block sizes. As an alternative, a dummy-prover may be used for development (see
[`development.md`](https://hackmd.io/S7hTv1EwSpWu8VCReDmsBg) for details).

Run prover:

```
zk prover
```

Make sure you have environment variables set right, you can check it by running: `zk env`. You should
see `* dev` inoutput.

## Troubleshooting

### SSL error: certificate verify failed

**Problem**. `zk init` fails with the following error:

```
Initializing download: https://universal-setup.ams3.digitaloceanspaces.com/setup_2%5E20.key
SSL error: certificate verify failed
```

**Solution**. Make sure that the version of `axel` on your computer is `2.17.10`.

### OpenSSL error on ubuntu

**Problem**. An error as the following:

```bash
thread 'main' panicked at 'OpenSSL library directory does not exist: /usr/lib/ss/lib', /home/usr/.cargo/registry/src/github.com-1ecc6299db9ec823/openssl-sys-0.9.65/build/main.rs:66:9
```

**Solution**. Try out the fixes [in this thread](https://github.com/sfackler/rust-openssl/issues/766), in particular:

```bash
export OPENSSL_LIB_DIR="/usr/lib/x86_64-linux-gnu"
export OPENSSL_INCLUDE="/usr/include/openssl"
```

### rmSync is not a function

**Problem**. `zk init` fails with the following error:

```
fs_1.default.rmSync is not a function
```

**Solution**. Make sure that the version of `node.js` installed on your computer is `14.14.0` or higher.

### Rust compilation problems

**Problem**. Compilation problems with lexical-core.

**Solution**. In `zksync/Cargo.toml` append at the end:

- `lexical-core = {git = 'https://github.com/Gelbpunkt/rust-lexical', branch = 'fix-warnings-and-update-deps'}`

Then run zk init again. The suggestion from discord, to try `cargo update lexical-core` does not work

**Problem**. Compilation problems with `bynarien` due to `Command Line Tools for Xcode v13.2+`.

Error:

```
In file included from <project_root>/sdk/binaryen/src/wasm/wasm-type.cpp:28:
<project_root>/sdk/binaryen/src/wasm-type.h:365:10: error: definition of implicit copy constructor for 'Tuple' is deprecated because it has a user-declared copy assignment operator [-Werror,-Wdeprecated-copy]
  Tuple& operator=(const Tuple&) = delete;
         ^
<project_root>/sdk/binaryen/src/wasm/wasm-type.cpp:51:51: note: in implicit copy constructor for 'wasm::Tuple' first required here
  TypeInfo(const Tuple& tuple) : kind(TupleKind), tuple(tuple) {}
                                                  ^
1 error generated.
make[2]: *** [src/wasm/CMakeFiles/wasm.dir/wasm-type.cpp.o] Error 1
make[1]: *** [src/wasm/CMakeFiles/wasm.dir/all] Error 2
make: *** [all] Error 2
error Command failed with exit code 2.
```

```bash
❯ clang++ --version
Apple clang version 13.1.6 (clang-1316.0.21.2)
Target: x86_64-apple-darwin21.4.0
```

**Solution**. Download the latest working `Command Line Tools for Xcode` from
[developer.apple.com](https://developer.apple.com/download/all/?q=command%20line%20tools). An apple account is required.

### Usage after building on `master`

**Problem**. After having built the project on `master`, the project is unable to verify transactions after building on
another branch.

**Solution**. Delete all related containers and the artifacts built.

Deleting the files built under the `binaryen` submodule.

Remove all the containers and images involved by running:

```bash
docker-compose down --rmi all -v --remove-orphans
```

Below some useful instructions to remove all the containers and images without `docker-compose`. Find the related
containers by executing:

```bash
docker container ls -a
```

And with the IDs from the previous steps (only related containers):

```bash
docker container rm <ID_1> <ID_2>...
```

Delete all related docker images. Find them by executing:

```bash
docker image ls -a
```

```bash
docker image remove <ID_1> <ID_2>...
```

If you have intermediate images that you wish to remove, please check out
[this thread](https://forums.docker.com/t/how-to-remove-none-images-after-building/7050).

After this, re-install the project. This will trigger sizable downloads, so expect this to take a while.

### Compiling issues M1

**_Problem_** The error could be `dyld[xxxx]: missing symbol called`.

With Mac M1, the clang version (c and c++ compiler) compiles for the ARM architecture only but there are some Rust
dependencies that have support for the intel x86_64 only.

**Solution** Use a terminal fully on [rosetta](https://support.apple.com/en-us/HT211861), for reference on how to do it,
follow this
[instructions](https://stackoverflow.com/questions/64882584/how-to-run-the-homebrew-installer-under-rosetta-2-on-m1-macbook/66299285#66299285).

You have to reinstall all dependencies but now with the x86_64 architecture.
