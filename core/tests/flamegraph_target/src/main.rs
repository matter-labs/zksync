use std::str::FromStr;
use structopt::StructOpt;

mod tree_target;

/// Target to analyze.
#[derive(Debug)]
enum Target {
    /// Merkle tree.
    Tree,
}

impl FromStr for Target {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let target = match s {
            "tree" | "merkle-tree" => Self::Tree,
            _ => return Err("Unknown taget. Available options are: tree"),
        };

        Ok(target)
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "flamegraph_target",
    about = "Binary for stress-testing zkSync components"
)]
struct Options {
    /// Name of the target to run.
    target: Target,
}

fn main() {
    let options = Options::from_args();

    // Not much currently, but we may want to add another targets in the future.
    match options.target {
        Target::Tree => {
            tree_target::analyze_tree();
        }
    }
}
