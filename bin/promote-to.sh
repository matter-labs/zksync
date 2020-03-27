#!/bin/bash

set -e

drone build promote $(git config --get remote.origin.url|sed -e 's/git@github.com:\(.*\).git/\1/') $2 $1
