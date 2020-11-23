#!/bin/bash

if ! [ -d grafonnet-lib ]; then
    git clone https://github.com/grafana/grafonnet-lib
fi

mkdir -p build

for template in $(ls *.jsonnet); do
    out=build/$(basename $template net)
    jsonnet -J grafonnet-lib $template > $out
done


