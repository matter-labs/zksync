import { Command } from 'commander';
import { loadConfig } from './config';
import { TimePeriod } from './utils';
import * as commands from './commands';

function print(object: any) {
    console.log(JSON.stringify(object, null, 4));
}

async function main() {
    try {
        const program = new Command();

        program.version('0.1.0').name('analytics').option('-n, --network <network>', 'select network');

        const config = loadConfig(program.network);

        program
            .command('current-balances')
            .description('output worth of tokens on operator balances in zkSync as ETH and USD')
            .action(async () => {
                const balancesInfo = await commands.currentBalances(config.network, config.operator_fee_address);
                print(balancesInfo);
            });

        program
            .command('fees')
            .description('output information about collected fees in the selected period')
            .requiredOption('--timeFrom <timeFrom>', "start of time period in format 'YYYY-MM-DDTHH:MM:SS'")
            .option('--timeTo <timeTo>', "end of time period in format 'YYYY-MM-DDTHH:MM:SS' (Default - current time)")
            .action(async (cmd: Command) => {
                const timePeriod = new TimePeriod(cmd.timeFrom, cmd.timeTo);
                const feesInfo = await commands.collectedFees(config.network, config.rest_api_address, timePeriod);
                print(feesInfo);
            });

        program
            .command('liquidations')
            .description(
                'output total amount of ETH accrued to the SENDER_ACCOUNT as a result of token liquidations during the specified period'
            )
            .requiredOption('--timeFrom <timeFrom>', "start of time period in format 'YYYY-MM-DDTHH:MM:SS'")
            .option('--timeTo [timeTo]', "end of time period in format 'YYYY-MM-DDTHH:MM:SS' (Default - current time)")
            .action(async (cmd: Command) => {
                const timePeriod = new TimePeriod(cmd.timeFrom, cmd.timeTo);
                const liquidationInfo = await commands.collectedTokenLiquidations(
                    config.network,
                    config.operator_fee_address,
                    timePeriod,
                    config.etherscan_api_key
                );
                print(liquidationInfo);
            });

        program.parse(process.argv);
    } catch (e) {
        console.error('Error: ', e);
        process.exit(1);
    }
}

main();
