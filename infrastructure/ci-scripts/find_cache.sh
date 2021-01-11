#!/bin/bash

rust_hash=$(md5sum Cargo.lock | awk '{ print $1 }')
if [ -d "$CACHE_DIR/$rust_hash" ]; then
  rm -rf ./target
  cp -r $CACHE_DIR/$rust_hash/target ./target
  echo "Load rust from cache"
fi

node_hash=$(md5sum yarn.lock | awk '{ print $1 }')
if [ -d "$CACHE_DIR/$node_hash" ]; then
  rm -rf ./node_modules
  cp -r $CACHE_DIR/$node_hash/node_modules ./node_modules
  echo "Load node from cache"
fi
