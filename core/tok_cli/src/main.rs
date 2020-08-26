mod token;
mod utils;

// External uses
use anyhow::{Context, Result};
use clap::{App, Arg, SubCommand};
use web3::{
    api::{Eth, Namespace},
    transports::Http,
};

// Local uses
use models::config_options::ConfigurationOptions;
use token::Token;
use utils::{get_matches_from_lines, run_external_command, str_to_address};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config_opts = ConfigurationOptions::from_env();

    let (_event_loop, transport) =
        Http::new(&config_opts.web3_url).expect("failed to start web3 transport");

    let eth = Eth::new(transport);

    let cmd_info = SubCommand::with_name("info")
        .about("Shows 'name', 'symbol', 'decimal' parameters for ERC20 token")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true),
        );

    let cmd_deploy_test = SubCommand::with_name("deploy-test")
        .about("Deploy test ERC20 token with 'name', 'symbol', 'decimal' parameters")
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("symbol")
                .short("s")
                .long("symbol")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("decimals")
                .short("d")
                .long("decimals")
                .takes_value(true),
        );

    let cmd_governance_add = SubCommand::with_name("governance-add")
        .about("Add token to the governance (work on testnet only).")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true),
        )
        .arg(
            // private key
            Arg::with_name("key")
                .short("k")
                .long("key")
                .takes_value(true),
        );

    let cmd_server_add = SubCommand::with_name("server-add")
        .about("Add ERC20 token to the server.")
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("symbol")
                .short("s")
                .long("symbol")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("decimals")
                .short("d")
                .long("decimals")
                .takes_value(true),
        );

    let cli = App::new("Tool for token addition")
        .author("Matter Labs")
        .subcommand(cmd_info)
        .subcommand(cmd_deploy_test)
        .subcommand(cmd_governance_add)
        .subcommand(cmd_server_add)
        .get_matches();

    match cli.subcommand() {
        ("info", Some(sub_m)) => {
            let addr = str_to_address(sub_m.value_of("address").context("Error 'address' value")?)?;

            let token = Token::get_info_about_token(addr, eth).await?;

            println!("{:#?}", token);
        }
        ("deploy-test", Some(sub_m)) => {
            let name = sub_m.value_of("name").context("Error 'name' value")?;

            let decimals = sub_m
                .value_of("decimals")
                .context("Error 'decimals' value")?
                .parse::<u8>()?;

            let symbol = sub_m.value_of("symbol").context("Error 'symbol' value")?;

            let token = Token::deploy_test_token(name, decimals, symbol).await?;

            println!("{:#?}", token);
        }
        ("governance-add", Some(sub_m)) => {
            let addr = str_to_address(sub_m.value_of("address").context("Error 'address' value")?)?;

            let key = {
                let str_key = sub_m.value_of("key").context("Expect key value")?;
                str_key.parse().context("Error parse private key value")?
            };

            Token::add_to_governance(addr, key).await?;
        }
        ("server-add", Some(sub_m)) => {
            let addr = str_to_address(sub_m.value_of("address").context("Error 'address' value")?)?;
            let name = sub_m.value_of("name").context("Error 'name' value")?;
            let decimals = sub_m
                .value_of("decimals")
                .context("Error 'decimals' value")?
                .parse::<u8>()?;
            let symbol = sub_m.value_of("symbol").context("Error 'symbol' value")?;

            let token = Token::new(addr, name, symbol, decimals)?;

            let endpoint_addr = config_opts.endpoint_http_server_address;
            let secret_auth = config_opts.secret_auth;

            let token_from_server = token.add_to_server(endpoint_addr, &secret_auth).await?;

            println!("{:#?}", token_from_server);
        }
        _ => {
            println!("Invalid command, try use '--help'");
        }
    }
    Ok(())
}
