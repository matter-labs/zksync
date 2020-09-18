mod cli;
mod token;
mod utils;

// External uses
use anyhow::{Context, Result};
use structopt::StructOpt;
use web3::{
    api::{Eth, Namespace},
    transports::Http,
};

// Local uses
use cli::App;
use token::Token;
use utils::{get_matches_from_lines, run_external_command, str_to_address};
use zksync_config::{AdminServerOptions, ConfigurationOptions};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config_opts = ConfigurationOptions::from_env();

    let (_event_loop, transport) =
        Http::new(&config_opts.web3_url).expect("failed to start web3 transport");

    let eth = Eth::new(transport);

    match App::from_args() {
        App::Info(cmd) => {
            let addr = str_to_address(&cmd.address)?;
            let token = Token::get_info_about_token(addr, eth).await?;

            println!("{:#?}", token);
        }
        App::DeployTest(cmd) => {
            let token = Token::deploy_test_token(&cmd.name, cmd.decimals, &cmd.symbol).await?;

            println!("{:#?}", token);
        }
        App::GovernanceAdd(cmd) => {
            let addr = str_to_address(&cmd.address)?;
            let key = cmd.key.parse().context("Error parse private key value")?;

            Token::add_to_governance(addr, key).await?;
        }
        App::ServerAdd(cmd) => {
            let addr = str_to_address(&cmd.address)?;
            let token = Token::new(addr, &cmd.name, &cmd.symbol, cmd.decimals);

            let admin_server_opts = AdminServerOptions::from_env();

            let endpoint_addr = admin_server_opts.admin_http_server_url;
            let secret_auth = admin_server_opts.secret_auth;

            let token_from_server = token.add_to_server(endpoint_addr, &secret_auth).await?;

            println!("{:#?}", token_from_server);
        }
    }
    Ok(())
}
