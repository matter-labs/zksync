import { Command } from 'commander';
import 'isomorphic-fetch';
import * as jwt from 'jsonwebtoken';

type Token = {
    id: null;
    address: string;
    symbol: string;
    decimals: number;
};

async function addToken(token: Token) {
    console.log('Adding new ERC20 token to server: ', token.address);

    const tokenEndpoint = `${process.env.ADMIN_SERVER_API_URL}/tokens`;
    const authToken = jwt.sign(
        {
            sub: 'Authorization'
        },
        process.env.ADMIN_SERVER_SECRET_AUTH,
        { expiresIn: '1m' }
    );

    const response = await fetch(tokenEndpoint, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${authToken}`,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(token, null, 2)
    });

    return await response.json();
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('server-add-erc20').description('add erc20 token to the zkSync server');

    program
        .command('add')
        .option('-a, --address <address>')
        .option('-s, --symbol <symbol>')
        .option('-d, --decimals <decimals>')
        .description('Adds a new token with a given fields to the zkSync server')
        .action(async (cmd: Command) => {
            const token: Token = {
                id: null,
                address: cmd.address,
                symbol: cmd.symbol,
                decimals: Number(cmd.decimals)
            };

            console.log(JSON.stringify(await addToken(token), null, 2));
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
