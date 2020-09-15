# Analytics - CLI to provide finance consuming reports

## Configuration

The application tries to locate a configuration file in the current working directory - `./.analytics-config.json`.

The configuration file contains the default network and a list of all networks with their parameters. Each network has the following arguments:

| Parameter | Description |
| :-- | :-- |
| `OPERATOR_COMMIT_ETH_ADDRESS` | Ethereum Address to be used for zkSync account managing the interaction with a contract on Ethereum |
| `OPERATOR_FEE_ETH_ADDRESS` | Ethereum Address to be used for zkSync account to collect fees |
| `REST_API_ADDR` | Address where the zkSync REST API is located |
| `ETHERSCAN_API_KEY` | Api-Key Token which uses for all API request to Etherscan   |

## Usage

```console
$ yarn start help

  Usage: analytics [options] [command]

  Options:
    -V, --version            output the version number
    -n, --network <network>  select network
    -h, --help               display help for command
    
  Commands:
    current-balances         output worth of tokens on operator balances in zkSync as ETH and USD
    fees [options]           output information about collected fees in the selected period
    liquidations [options]   output total amount of ETH accrued to the SENDER_ACCOUNT as a result of token liquidations during the specified period
    help [command]           display help for command

```

## Commands

All output printed to stdout is strict JSON

### Options/flags

- --network \<networt\> (Default: from config file)  
select a network from the list of the configuration file
- --timeFrom \<time\>   
start of time period in format 'YYYY-MM-DDTHH:MM:SS'
- --timeTo \<time\> (Default - current time)  
end of time period in format 'YYYY-MM-DDTHH:MM:SS' 

### Current balances reports

```console
$ yarn start current-balances 
```

Output current balance of on operator balances in zkSync as ETH and USD.

The report contains information about all tokens that are supported in zkSync.

### Collected fees reports

```console
$ yarn start fees [--timeFrom <time>] [--timeTo <time>]
```
Output such information:
- amount of ETH spent for `commit`, `verify` and `completeWithdrawals` operations in L1 and it's equivalent in USD (at present moment)
- information about fees collected during this period in each token and their equivalent in ETH and their equivalent in ETH and USD (at present moment)

### Liquidations reports

```console
$ yarn start liquidations [--timeFrom <time>] [--timeTo <time>]
```

The total amount of ETH accrued as a result of token liquidations during the specified period.

## Testing

```console
$ yarn test 
```

## Example usage

```console 
$ yarn start current-balances 
{
    "total": {
        "eth": 101.23231,
        "usd": 37018.63
    },
    "BAT": {
        "amount": 10000.32,
        "eth": 320.01024,
        "usd": 0.88726
    },
    ...
}
$ yarn start fees --timeFrom 2020-09-15T00:00:00
{

}
$ yarn start liquidations --timeFrom 2020-09-14 --timeTo 2020-09-15
{
    "Total amount of ETH": 37.157017451243775
}
```