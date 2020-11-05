//! Load test aims the following purposes:
//! - spamming the node with a big amount of transactions to simulate the big pressure;
//! - measuring the metrics of the node txs processing progress;
//! - making many API requests to simulate a typical user workflow.
//! - quick filling the node's database with a lot of the real-world data.
//!
//! The behavior of the loadtest is flexible and determined by different "scenarios":
//! every scenario is basically a function which interacts with a node according to some rules.
//! All scenarios can be run simultaneously in any combination.
//!
//! Currently supported scenarios:
//!
//! - Transfer - spamming the node with a big amount of transfer transactions.
//!
//! - withdraw - performs several withdraw / deposit operations.
//!
//! - full_exit (incomplete) - performs several full_exit / deposit operations.
//!

// Built-in import
use std::path::PathBuf;
// External uses
use chrono::{SecondsFormat, Utc};
use colored::*;
use structopt::StructOpt;
// Workspace uses
use zksync_config::ConfigurationOptions;
// Local uses
use loadtest::{Config, FiveSummaryStats, LoadtestExecutor};

/// An utility for simulating a load similar to a real one.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct LoadtestOpts {
    /// Path to a load test configuration file.
    #[structopt(short = "p", long)]
    config_path: Option<PathBuf>,
    /// Print the results as json file.
    #[structopt(long)]
    json_output: bool,
    /// The path to the load test results
    #[structopt(short = "o", long)]
    out_dir: Option<PathBuf>,
}

macro_rules! pretty_fmt {
    ($ms:expr) => {
        match ($ms as f64) {
            ms if ms < 1_000_f64 => format!("{:.1}µs", ms),
            ms if ms < 1_000_000_f64 => format!("{:.2}ms", ms / 1_000_f64),
            ms => format!("{:.2}s", ms / 1_000_000_f64),
        }
    };
}

fn print_stats_summary(name: impl AsRef<str>, summary: Option<&FiveSummaryStats>) {
    println!("    {}:", name.as_ref().green());
    if let Some(summary) = summary {
        println!(
            "        [ {} {} {} {} {} ] (std_dev = {})",
            pretty_fmt!(summary.min).dimmed(),
            pretty_fmt!(summary.lower_quartile),
            pretty_fmt!(summary.median).bright_blue().bold(),
            pretty_fmt!(summary.upper_quartile),
            pretty_fmt!(summary.max).dimmed(),
            pretty_fmt!(summary.std_dev).yellow()
        );
    }
}

fn print_counters(failed: usize, total: usize) {
    if failed > 0 {
        println!(
            "          {} of {} requests have been {}.",
            failed.to_string().red(),
            total,
            "failed".red(),
        );
    } else {
        println!(
            "          All of {} requests have been {}.",
            total,
            "successful".green()
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let env_config = ConfigurationOptions::from_env();

    let opts = LoadtestOpts::from_args();

    let config = opts
        .config_path
        .map(Config::from_toml)
        .transpose()?
        .unwrap_or_default();
    let out_dir = opts.out_dir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("loadtest")
            .join(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false))
    });
    std::fs::create_dir_all(&out_dir)?;

    loadtest::init_session(out_dir).await?;

    let executor = LoadtestExecutor::new(config, env_config).await?;
    let report = executor.run().await?;

    loadtest::finish_session(&report).await?;

    if opts.json_output {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Loadtest finished.");

        println!("Statistics for scenarios:");
        for (category, stats) in &report.scenarios.summary {
            print_stats_summary(category, Some(stats));
        }
        print_counters(
            report.scenarios.failed_txs_count,
            report.scenarios.total_txs_count,
        );

        println!("Statistics for API tests:");
        for (category, stats) in &report.api {
            print_stats_summary(category, stats.summary.as_ref());
            print_counters(stats.failed_requests_count, stats.total_requests_count);
        }
    }

    Ok(())
}
