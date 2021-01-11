#!/bin/bash

rust_hash=$(md5sum Cargo.lock | awk '{ print $1 }')
if [ ! -d "$CACHE_DIR/$rust_hash" ]; then
  mkdir -p "$CACHE_DIR/$rust_hash"
fi
rm -rf $CACHE_DIR/$rust_hash/target
cp -r ./target $CACHE_DIR/$rust_hash/target

node_hash=$(md5sum yarn.lock | awk '{ print $1 }')
if [ -d "$CACHE_DIR/$node_hash" ]; then
  mkdir -p "$CACHE_DIR/$node_hash"
fi
rm -rf $CACHE_DIR/$node_hash/node_modules
cp -r  ./node_modules $CACHE_DIR/$node_hash/node_modules
