import 'isomorphic-fetch';
import { Command } from 'commander';
import * as commands from './commands';
import { loadConfig } from './config';
import * as types from './types';

function print(object: any) {
    console.log(JSON.stringify(object, null, 2));
};

(async () => {
    const program = new Command();

    program
        .version("0.1.0")
        .name("analytics")
        .option("-n, --network <network>", "select network");
    
    const config = loadConfig(program.network);

    program
        .command("current-balances")
        .description("output worth of tokens on operator balances in zkSync as ETH and USD")
        .action(async () => {
            print(await commands.currentBalances(config.network, config.operator_commit_address));
        });

    program
        .command("fees")
        .description("output information about collected fees in the selected period")
        .requiredOption("--timeFrom <timeFrom>", "start of time period in format 'DD-MM-YYYY HH:MM:SS'")
        .option("--timeTo <timeTo>", "end of time period in format 'DD-MM-YYYY HH:MM:SS' (Default - current time)")
        .action(async (cmd: Command) => {
            const timePeriod = new types.TimePeriod(cmd.timeFrom, cmd.timeTo);
            print(await commands.collectedFees(config.network, config.rest_api_address, timePeriod));
        });
    
    program
        .command("liquidations")
        .description("output total amount of ETH accrued to the SENDER_ACCOUNT as a result of token liquidations during the specified period")
        .requiredOption("--timeFrom <timeFrom>", "start of time period in format 'DD-MM-YYYY HH:MM:SS'")
        .option("--timeTo [timeTo]", "end of time period in format 'DD-MM-YYYY HH:MM:SS' (Default - current time)")
        .action(async () => {
            print(await commands.collectedTokenLiquidations());
        });

    program.parse(process.argv);
})();
