#!/bin/bash

set -e

if ! [ -d grafonnet-lib ]; then
    git clone https://github.com/grafana/grafonnet-lib
fi

mkdir -p build

for template in $(ls dashboards); do
    echo -n "Building $template ... "
    dashboard=build/$(basename $template net)
    jsonnet dashboards/$template > $dashboard
    echo Done
done

# AUTH must be in the form `login:password`
# We should move to using API Keys instead
[ -z $AUTH ] && echo 'Set $AUTH to deploy dashboards' && exit

for template in $(ls dashboards); do
    dashboard=$(basename $template net)
    echo -n "Deploying $dashboard ... "
    curl -X POST -H "Content-Type: application/json" \
        -d  "$(jq '{"folderId": 0, "overwrite": true, "dashboard": .}' build/$dashboard)" \
        https://$AUTH@grafana.test.zksync.dev/api/dashboards/db 2> /dev/null | jq .status
done
