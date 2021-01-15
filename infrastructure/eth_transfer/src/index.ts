import { Command } from 'commander';
import { ethers, Wallet } from 'ethers';

const program = new Command();
program.version('0.0.1');

program
    .option('-pk, --private-key <private-key>', 'private key of the sender')
    .option('-t, --target <target>', 'address of the target account')
    .option('-n, --network <network>', 'eth network')
    .option('-a, --amount <amount>', 'amount of the ETH to be sent');

program.parse(process.argv);

function getProvider(network: string) {
    if (network === 'localhost') {
        return new ethers.providers.JsonRpcProvider('http://localhost:8545');
    }

    return ethers.providers.getDefaultProvider(network);
}

async function main() {
    const { privateKey, target, amount, network } = program;

    const provider = getProvider(network || 'mainnet');
    const wallet = new Wallet(privateKey).connect(provider);

    let tx = {
        to: target,
        value: ethers.utils.parseEther(amount)
    };

    try {
        const txResponse = await wallet.sendTransaction(tx);
        console.log('Transaction was sent! Hash: ', txResponse.hash);
    } catch (err) {
        console.log('Failed to send tx. Reason: ', err.message || err);
    }
}

main().catch((err: Error) => {
    console.error('Error:', err.message || err);
    process.exitCode = 1;
});
