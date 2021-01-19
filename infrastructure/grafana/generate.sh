#!/bin/sh

set -e

case "$1" in
    mainnet) domain=grafana.zksync.io ;;
    testnet) domain=grafana.test.zksync.dev ;;
    *)       echo 'Specify where to deploy to - testnet/mainnet' && exit 1 ;;
esac


if ! [ -d grafonnet-lib ]; then
    git clone https://github.com/grafana/grafonnet-lib
fi

mkdir -p build

# AUTH must be in the form `login:password`
# We should move to using API Keys later
if [ -z "$AUTH" ]; then
    echo 'Set $AUTH to deploy dashboards'
else
    folderId=$(curl https://${AUTH}@${domain}/api/folders 2> /dev/null |
               jq '.[] | if .title == "Metrics" then .id else null end | numbers')
    echo Folder ID: $folderId
fi

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
        -d  "$(jq "{folderId: $folderId, overwrite: true, dashboard: .}" build/$dashboard)" \
        https://${AUTH}@${domain}/api/dashboards/db 2> /dev/null | jq .status
done
