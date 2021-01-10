#!/bin/bash

rust_hash=$(md5sum Cargo.toml | awk '{ print $1 }')
if [ ! -d "$CACHE_DIR/$rust_hash" ]; then
  mkdir -p "$CACHE_DIR/$rust_hash"
fi
rm -rf $CACHE_DIR/$rust_hash/target
cp -r ./target $CACHE_DIR/$rust_hash/target
