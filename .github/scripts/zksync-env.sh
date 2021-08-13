#!/bin/bash
## This script updates server-env-custom (merges the etc/env/xxx file with configmap)
#   loadtest (without parameters) - combined env
#   --repo [loadtest] - env from file
#   --kube [loadtest] - env from configmap
#   --diff [loadtest] - show the diff to be applied to the configmap
#   --merge [loadtest] - writes merged configmap with merged env values
#   --update-from fromfile [loadtest] - updates loadtestet env with values fromfile
#   --diff-from fromfile [loadtest] - show diff for loadtestet env with values fromfile

set -e

serverEnv="server-env-custom"
repoRoot=`cd $( dirname "${BASH_SOURCE[0]}" )/../.. >/dev/null 2>&1 && pwd`

cmd="$1"
opts=""

if ( echo "--update-from --diff-from" | grep -qw "\\$cmd" ); then
  fromfile="$2"
  namespace="${ZKSYNC_ENV:-$3}"
else
  namespace="${ZKSYNC_ENV:-$2}"
fi

[ -z $namespace ] || opts="$opts -n $namespace"

kube_env() {
  if ( kubectl $opts get cm $serverEnv &> /dev/null ); then
    kubectl $opts get cm $serverEnv -o go-template --template='{{ range $k,$v := .data}}{{ printf "%s=%s" $k $v }}{{"\n"}}{{end}}'
  fi
}

##
repo_env() {
  . $repoRoot/etc/env/$namespace.env
  export $(cut -d= -f1 etc/env/$namespace.env | sed -r '/^\s*#/d')
  env | sed "/^\($1\)/d"
}

cmDiff() {
  kubectl $opts create cm $serverEnv --from-env-file="$tmpfile" --dry-run -o yaml | kubectl diff -f - || :
}

cleanup() {
  if [ -n "$tmpfile" ]; then
    rm -f "$tmpfile"
  fi
}

trap cleanup EXIT

## We call ourself - carefull!
case $cmd in
  --kube)
    kube_env
    ;;
  --repo)
    # current env to sanitize
    sanitize=$(env | cut -d= -f1 | sed ':a;N;$!ba;s/\n/\\\|/g')
    repo_env "$sanitize"
    ;;
  --diff)
      tmpfile=$(mktemp -u)
      bash $0 $2 > "$tmpfile"
      cmDiff
    ;;
  --merge)
    tmpfile=$(mktemp -u)
    bash $0 $2 > "$tmpfile"
    
    # overwrites configmap!
    outp=$(cmDiff)
    if ( echo "$outp" | grep -Fq '+++' ); then
      kubectl $opts create cm $serverEnv --from-env-file="$tmpfile" --dry-run -o yaml | \
        kubectl apply -f -
    elif [ -n "$outp" ]; then
      # write a error (since no diff)
      echo $outp
      exit 2
    fi
    ;;
  --update-from)
    tmpfile=$(mktemp -u)
    { cat $fromfile; bash $0 --kube $namespace; } | sort -u -t '=' -k 1,1 > "$tmpfile"

    # overwrites configmap!
    outp=$(cmDiff)
    if ( echo "$outp" | grep -Fq '+++' ); then
      kubectl $opts create cm $serverEnv --from-env-file="$tmpfile" --dry-run -o yaml | \
        kubectl apply -f -
    elif [ -n "$outp" ]; then
      # write a error (since no diff)
      echo $outp
      exit 2
    fi
    ;;
  --diff-from)
    tmpfile=$(mktemp -u)
    { cat $fromfile; bash $0 --kube $namespace; } | sort -u -t '=' -k 1,1 > "$tmpfile"
    cmDiff
    ;;
  *)
    # Combine two outputs.
    # We favour output from kube!
    #
    { bash $0 --kube $1; bash $0 --repo $1; } | sort -u -t '=' -k 1,1
    ;;
esac
