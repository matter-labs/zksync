# zkSync flamegraph target

This binary is a playground for analyzing the performance of the zkSync components, using tools
such as [perf] and [cargo-flamegraph].

While analyzing the performance of the application overall may be useful, it doesn't really help
when you want to analyze one particular component when it takes ~100% of the runtime, in order to
see weak points clearly.

[perf]: https://perf.wiki.kernel.org/index.php/Main_Page
[cargo-flamegraph]: https://github.com/flamegraph-rs/flamegraph

## Design notes

Since the goal is to avoid *everything* that is not related to the analysis of the particular component,
try to avoid "hidden" runtime logic such as async runtimes (e.g. `tokio::main`) or logging (e.g. `vlog`)
in this crate. If the code you are going to analyze requires `async` support, explicitly create the runtime
for said code only.

Target code should be written in a manner that stresses component in the most possible way.
If your code needs initialization, separate it from the actual stressing logic, so that in the resulting
flamegraph they will be easily distinguishable.
