#!/bin/bash

rust_hash=$(md5sum Cargo.lock | awk '{ print $1 }')
if [ -d "$CACHE_DIR/$rust_hash" ]; then
  rm -rf ./target
  cp -r $CACHE_DIR/$rust_hash/target ./target
  echo "Load from cache"
fi

