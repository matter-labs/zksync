# Installing dependencies

## `Docker`

Install `docker`. It is recommended to follow the instructions from the
[official site](https://docs.docker.com/install/).

Installing `docker` via `snap` or from the default repository can cause troubles.

You need to install both `docker` and `docker-compose`.

**Note:** On linux you may encounter the following error when you’ll try to work with `zksync`:

```
ERROR: Couldn't connect to Docker daemon - you might need to run `docker-machine start default`.
```

If so, you **do not need** to install `docker-machine`. Most probably, it means that your user is not added to
the`docker` group. You can check it as follows:

```bash
docker-compose up # Should raise the same error.
sudo docker-compose up # Should start doing things.
```

If the first command fails, but the second succeeds, then you need to add your user to the `docker` group:

```bash
sudo usermod -a -G docker your_user_name
```

After that, you should logout and login again (user groups are refreshed after the login). The problem should be
solvedat this step.

If logging out does not help, restarting the computer should.

## `Node` & `Yarn`

1. Install `Node` (requires version 14.14.0 or higher). Since our team attempts to always use the latest LTS version
   of`Node.js`, we suggest you to install [nvm](https://github.com/nvm-sh/nvm). It will allow you to
   update `Node.js`version easily in the future.
2. Install `yarn`. Instructions can be found on the [official site](https://classic.yarnpkg.com/en/docs/install/). Check
   if `yarn` is installed by running `yarn -v`.If you face any problems when installing `yarn`, it might be the case
   that your package manager installed the wrong package.Make sure to thoroughly follow the instructions above on the
   official website. It contains a lot of troubleshootingguides in it.
3. Run `yarn global add @vue/cli-service`

## `Axel`

Install `axel` for downloading keys:

On mac:

```bash
brew install axel
```

On debian-based linux:

```bash
sudo apt-get install axel
```

Check the version of `axel` with the following command:

```
axel --version
```

Make sure the version is `2.17.10`.

## `Rust`

Install the latest `rust` version.

Instructions can be found on the [official site](https://www.rust-lang.org/tools/install).

Verify the `rust` installation:

```bash
rustc --version
rustc 1.48.0 (7eac88abb 2020-11-16)
```

### `lld`

Optionally, you may want to optimize the build time with the LLVM linker, `lld`. Make sure you have it installed and
append `"-C", "link-arg=-fuse-ld=lld"` to the `rustflags` in your `.cargo/config` file, so it looks like this:

```
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "link-arg=-fuse-ld=lld",
]
```

**Warning:** This is only viable for linux since `lld` doesn’t work on mac.

## PSQL

Install `psql` CLI tool to interact with postgres.

On debian-based linux:

```bash
sudo apt-get install postgresql-client
```

## `Diesel` CLI

Install `[diesel](https://diesel.rs/)` CLI (it is used for migrations management only):

```bash
cargo install diesel_cli --no-default-features --features postgres
```

If at the install step you get the linkage errors, install the development version of `libpq`.

On debian-based linux:

```bash
sudo apt-get install libpq-dev
```

If you still see the errors, install the `build-essential` package. On debian-based linux:

```bash
sudo apt install build-essential
```

## `sqlx` CLI

Also, we need `[sqlx](https://github.com/launchbadge/sqlx)` CLI (it is used to generate database wrappers):

```bash
cargo install --version=0.2.0 sqlx-cli
```

If you face an error `Could not find directory of OpenSSL installation`, then you should do the following.
Theinstruction is targeted on debian-based Linux, but generally, the steps are similar for all OS.

- Install `libssl-dev`:

```
sudo apt install libssl-dev
```

- Install OpenSSL. Here is [the instruction for Ubuntu](https://www.spinup.com/installing-openssl-on-ubuntu/), but
  thesteps should be similar for the debian-based Linux distros.
- Add `OPENSSL_DIR` variable to your environment. This would typically be `/usr/local/ssl`. You can do this by addingthe
  following line to your shell profile file (e.g. `~/.bash_profile`):

```bash
export OPENSSL_DIR=/usr/local/ssl
```

- Install `package-config`:

```bash
sudo apt-get install -y pkg-config
```

## `solc`

You have to install `solc` v0.5.16. Instructions can be found at
[readthedocs](https://solidity.readthedocs.io/en/v0.6.2/installing-solidity.html).

The simplest option for linux is to use `snap`.

For mac you can install it as follows:

```bash
brew update
brew upgrade
brew tap ethereum/ethereum
brew install solidity@5
```

If you're Arch user, download the archived version from [here](https://archive.archlinux.org/packages/s/solidity/) and
install it

```bash
pacman -U solidity-0.5.14-1-x86_64.pkg.tar.xz
```

Finally, to prevent pacman from upgrading it, add this line to your /etc/pacman.conf

```
IgnorePkg = solidity
```

## drone cli

drone cli used to create promotion jobs [described here](https://docs.drone.io/cli/install/).

## `cmake`

Required by `binaryen` to build C++ sources. In order to speed it up, you might want to install `clang` and `lld` too.

```bash
sudo apt-get install cmake clang lld
```

## Environment

Edit the lines below and add them to your shell profile file (e.g. `~/.bash_profile`):

```bash
# Add path here:
export ZKSYNC_HOME=/path/to/zksync

export PATH=$ZKSYNC_HOME/bin:$PATH

# If you're like me, uncomment:
# cd $ZKSYNC_HOME
```
