#!/bin/bash

confVar="$1"
mkdir -p ~/.kube &> /dev/null

if [ -n "$confVar" ]; then
  read KUBECONF <<< $(eval echo \$$confVar)
else
  : ${KUBECONF?KUBECONF or envvar name must be provided}
fi

echo $KUBECONF | base64 -d > ~/.kube/config
