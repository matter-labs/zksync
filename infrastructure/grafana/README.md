# Generating Grafana dashboards with `jsonnet`

## Motivation

Configuring Grafana's dashboards by hand can be very time-consuming. Especially, if we have ~170 metrics. Also, it
should be preferred to store the configs in code anyways.

Although Grafana stores all its dashboards in `json`, the data there is hardly readable and very repetitive.

## Solution: `jsonnet`

[`jsonnet`](https://jsonnet.org) is a superset of `json` that aims to be an easy language for generating `json`.

Grafana supports `jsonnet` for configuration via the library [`grafonnet`](https://github.com/grafana/grafonnet-lib),
which is used to configure our dashboards.

You can familiarize yourself with `jsonnet` on their official website, although it is not necessary (assuming the goal
is to add/change a metric) given the simplicity of the language.

## Usage

**Dependencies**: `jsonnet`, `jq`

Adding a metric is trivial, there are plenty of examples in `dashboards/` folder. Simply add the metric name to the
`metrics` array.

To create a new dashboard, assuming it will contain graphs of running averages of metrics provided by
`metrics::histogram!`, create a new `.jsonnet` file in the `dashboards/` folder. Use `G.panel` and `G.dashboard`
functions to configure your dashboard.

To (re)build and (re)deploy the dashboard, run the `./generate.sh` script with `$AUTH` env variable set to your Grafana
credentials, like so:

```
$ AUTH=login:password ./generate.sh
Building metrics.jsonnet ... Done
Deploying metrics.json ... "success"
```

If you don't see the message that dashboard is deployed, `touch` the source file and try again.
