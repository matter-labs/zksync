#!/bin/bash

if ! [ -d grafonnet-lib ]; then
    git clone https://github.com/grafana/grafonnet-lib
fi

mkdir -p build

for template in $(ls *.jsonnet); do
    echo -n "Building $template ... "
    out=build/$(basename $template net)
    jsonnet $template > $out
    echo Done
done


