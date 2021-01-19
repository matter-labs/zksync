#!/bin/bash -ue

export KEYBASE_ALLOW_ROOT=1
keybase oneshot

REPO=$1

git clone $REPO $HOME/secrets
ls $HOME/secrets
cp -r $HOME/secrets/* ./etc/
rm -rf $HOME/secrets
