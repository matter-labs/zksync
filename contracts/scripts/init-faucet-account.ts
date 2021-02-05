import { ArgumentParser } from 'argparse';
import * as fs from 'fs';
import * as path from 'path';
import * as zksync from 'zksync';
import { ethers } from 'ethers';
import { web3Provider } from './utils';

const DEPOSIT_AMOUNT = ethers.utils.parseEther('10000000000');
const network = process.env.CHAIN_ETH_NETWORK;

const provider = web3Provider();
const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

async function main() {
    const parser = new ArgumentParser({
        version: '0.1.0',
        addHelp: true
    });
    parser.addArgument('--deployerPrivateKey', { required: false, help: 'Wallet used to deploy contracts' });
    parser.addArgument('--faucetPrivateKey', { required: false, help: 'Wallet used as faucet' });
    const args = parser.parseArgs(process.argv.slice(2));

    const deployerEthWallet = args.deployerPrivateKey
        ? new ethers.Wallet(args.deployerPrivateKey, provider)
        : ethers.Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/1").connect(provider);
    const faucetEthWallet = args.faucetPrivateKey
        ? new ethers.Wallet(args.faucetPrivateKey, provider)
        : ethers.Wallet.fromMnemonic(ethTestConfig.mnemonic, "m/44'/60'/0'/0/2").connect(provider);

    const syncProvider = await zksync.getDefaultProvider(network as zksync.types.Network);
    const deployerWallet = await zksync.Wallet.fromEthSigner(deployerEthWallet, syncProvider);
    const faucetWallet = await zksync.Wallet.fromEthSigner(faucetEthWallet, syncProvider);

    console.log('Faucet ETH_PRIVATE_KEY', faucetEthWallet.privateKey);
    const TOKEN_ADDRESS = syncProvider.tokenSet.resolveTokenAddress('MLTT');
    const ABI = [
        {
            constant: false,
            inputs: [
                {
                    internalType: 'address',
                    name: '_to',
                    type: 'address'
                },
                {
                    internalType: 'uint256',
                    name: '_amount',
                    type: 'uint256'
                }
            ],
            name: 'mint',
            outputs: [
                {
                    internalType: 'bool',
                    name: '',
                    type: 'bool'
                }
            ],
            payable: false,
            stateMutability: 'nonpayable',
            type: 'function'
        }
    ];
    if (process.env.CHAIN_ETH_NETWORK !== 'localhost') {
        const erc20Mintable = new ethers.Contract(TOKEN_ADDRESS, ABI, deployerEthWallet);
        const mintTx = await erc20Mintable.mint(deployerEthWallet.address, DEPOSIT_AMOUNT);
        await mintTx.wait();
        console.log('Mint successful');
    }

    const deposit = await deployerWallet.depositToSyncFromEthereum({
        depositTo: faucetEthWallet.address,
        token: 'MLTT',
        amount: DEPOSIT_AMOUNT,
        approveDepositAmountForERC20: true
    });
    await deposit.awaitReceipt();
    console.log('Deposit successful');

    if (!(await faucetWallet.isSigningKeySet())) {
        const setSigningKey = await faucetWallet.setSigningKey({ feeToken: 'MLTT', ethAuthType: 'ECDSA' });
        await setSigningKey.awaitReceipt();
        console.log('Signing key is set');
    }
    console.log('Faucet account is prepared');
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
