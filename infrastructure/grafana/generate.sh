#!/bin/sh

set -e

if ! [ -d grafonnet-lib ]; then
    git clone https://github.com/grafana/grafonnet-lib
fi

mkdir -p build

# AUTH must be in the form `login:password`
# We should move to using API Keys later
[ -z "$AUTH" ] && echo 'Set $AUTH to deploy dashboards'

for template in $(ls dashboards); do
    dashboard=$(basename $template net)
    # check if source is newer than target, otherwise we don't have to do anything
    [ "build/$dashboard" -nt "dashboards/$template" ] && continue
    echo -n "Building $template ... "
    jsonnet dashboards/$template > build/$dashboard
    echo Done
    [ -z "$AUTH" ] && continue
    echo -n "Deploying $dashboard ... "
    curl -X POST -H "Content-Type: application/json" \
        -d  "$(jq '{"folderId": 0, "overwrite": true, "dashboard": .}' build/$dashboard)" \
        https://$AUTH@grafana.test.zksync.dev/api/dashboards/db 2> /dev/null | jq .status
done
