# Running the application

This document covers common scenarios of launching zkSync applications set locally.

## Prerequisites

Prepare dev environment prerequisites: see

[Installing dependencies](./setup-dev.md)

## Setup local dev environment

Setup:

```
zk # installs and builds zk itself
zk init
```

During the first initialization you have to download around 8 GB of setup files, this should be done once. If you have a
problem on this step of the initialization, see help for the `zk run plonk-setup` command.

If you face any other problems with the `zk init` command, go to the [Troubleshooting](##Troubleshooting) section at the
end of this file. There are solutions for some common error cases.

To completely reset the dev environment:

- Stop services:

  ```
  zk down
  ```

- Repeat the setup procedure above

If `zk init` has already been executed, and now you only need to start docker containers (e.g. after reboot),
simplylaunch:

```
zk up
```

## (Re)deploy db and contraсts

```
zk contract redeploy
```

## Environment configurations

Env config files are held in `etc/env/`

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

Server is configured using env files in `./etc/env` directory. After the first initialization,
file `./etc/env/dev.env`will be created. By default, this file is copied from the `./etc/env/dev.env.example` template.

Server can produce block of different sizes, the list of available sizes is determined by
the`SUPPORTED_BLOCK_CHUNKS_SIZES` environment variable. Block sizes which will actually be produced by the server can be
configured using the `BLOCK_CHUNK_SIZES` environment variable.

Note: for proof generation for large blocks requires a lot of resources and an average user machine is only capable
ofcreating proofs for the smallest block sizes. As an alternative, a dummy-prover may be used for development
(see`[development.md](https://hackmd.io/S7hTv1EwSpWu8VCReDmsBg)` for details).

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

### rmSync is not a function

**Problem**. `zk init` fails with the following error:

```
fs_1.default.rmSync is not a function
```

**Solution**. Make sure that the version of `node.js` installed on your computer is `14.14.0` or higher.
