# Build RSKj docker image from sourcecode

We created this Dockerfile so M1 Mac users can run the RSKj docker container. Currently there's no arm64 ubuntu package
available for RSKj so the Dockerfile located in /rskj/Dockerfile will fail when trying to fetch that Ubuntu package.
This alternative docker image will build the jar from the sourcecode.

## How to use

Copy the `Dockerfile` and `supervisord.conf` files located in `/rskj/from_source` to `/rskj` (replacing the already
existing files).

It is recommended that you run `docker-compose build --no-cache`
