#!/bin/bash
terminating=""

# exit when the second signal received
graceful_exit() {
  if [ -n "$terminating" ]; then
    exit $code
  fi
  terminating="yes"
}

trap graceful_exit SIGINT SIGTERM SIGHUP

while : ; do
  /bin/prover-entry.sh; code=$?
  [ "$terminating" = "yes" ] && exit $code
  # restart prover on failure
  [ "$code" -eq 0 ] || exit $code
done
