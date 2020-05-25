#!/bin/sh
# works using annotations from https://github.com/andreykaipov/active-standby-controller
grep 'qoqo.dev/pod-designation="active"' /etc/podinfo/labels &> /dev/null