import { Command } from 'commander';
import { ethers } from 'ethers';
import { web3Provider } from './utils';

const provider = web3Provider();

type Token = {
    address: string;
    name: string | null;
    symbol: string | null;
    decimals: number | null;
};

const TokenInterface = [
    'function name() view returns (string)',
    'function symbol() view returns (string)',
    'function decimals() view returns (uint)'
];

async function tokenInfo(address: string): Promise<Token> {
    const contract = new ethers.Contract(address, TokenInterface, provider);

    return {
        address: address,
        name: await contract.name().catch(() => null),
        symbol: await contract.symbol().catch(() => null),
        decimals: await contract
            .decimals()
            .then((decimals) => Number(decimals))
            .catch(() => null)
    };
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('deploy-erc20').description('deploy testnet erc20 token');

    program.command('info <address>').action(async (address: string) => {
        console.log(JSON.stringify(await tokenInfo(address), null, 2));
    });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
