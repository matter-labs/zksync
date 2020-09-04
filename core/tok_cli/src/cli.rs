//! Command-line interface for the token tool.

use structopt::StructOpt;

/// CLI parameters.
#[derive(Debug, StructOpt)]
#[structopt(name = "tok_cli", about = "Tool for token addition")]
pub enum App {
    #[structopt(
        name = "info",
        about = "Shows 'name', 'symbol', 'decimal' parameters for ERC20 token"
    )]
    Info(InfoOpts),
    #[structopt(
        name = "deploy-test",
        about = "Deploy test ERC20 token with 'name', 'symbol', 'decimal' parameters"
    )]
    DeployTest(DeployTestOpts),
    #[structopt(
        name = "governance-add",
        about = "Add token to the governance (work on testnet only)"
    )]
    GovernanceAdd(GovernanceAddOpts),
    #[structopt(name = "server-add", about = "Add ERC20 token to the server")]
    ServerAdd(ServerAddOpts),
}

#[derive(Debug, StructOpt)]
pub struct InfoOpts {
    #[structopt(name = "address", short = "a", long = "address")]
    pub address: String,
}

#[derive(Debug, StructOpt)]
pub struct DeployTestOpts {
    #[structopt(name = "address", short = "n", long = "name")]
    pub name: String,
    #[structopt(name = "symbol", short = "s", long = "symbol")]
    pub symbol: String,
    #[structopt(name = "decimals", short = "d", long = "decimals")]
    pub decimals: u8,
}

#[derive(Debug, StructOpt)]
pub struct GovernanceAddOpts {
    #[structopt(name = "address", short = "a", long = "address")]
    pub address: String,
    // private key
    #[structopt(name = "key", short = "k", long = "key")]
    pub key: String,
}

#[derive(Debug, StructOpt)]
pub struct ServerAddOpts {
    #[structopt(name = "address", short = "a", long = "address")]
    pub address: String,
    #[structopt(name = "name", short = "n", long = "name")]
    pub name: String,
    #[structopt(name = "symbol", short = "s", long = "symbol")]
    pub symbol: String,
    #[structopt(name = "decimals", short = "d", long = "decimals")]
    pub decimals: u8,
}
