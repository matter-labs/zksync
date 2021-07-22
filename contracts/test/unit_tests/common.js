const ethers = require('ethers');
const { use } = require('chai');
const { solidity, deployContract, MockProvider } = require('ethereum-waffle');

const IERC20_INTERFACE = require('@openzeppelin/contracts/build/contracts/IERC20');
const { rawEncode } = require('ethereumjs-abi');
const DEFAULT_REVERT_REASON = 'VM did not revert';

// For: geth

// const provider = new ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL);
// const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
// const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

// For: ganache

const provider = new MockProvider({ ganacheOptions: { gasLimit: '8000000', gasPrice: '1' } });
const [wallet, wallet1, wallet2, exitWallet] = provider.getWallets();

use(solidity);

async function deployTestContract(file) {
    try {
        return await deployContract(wallet, require(file), [], {
            gasLimit: 6000000
        });
    } catch (err) {
        console.log('Error deploying', file, ': ', err);
    }
}

async function deployProxyContract(wallet, proxyCode, contractCode, initArgs, initArgsValues) {
    try {
        const initArgsInBytes = await rawEncode(initArgs, initArgsValues);
        const contract = await deployContract(wallet, contractCode, [], {
            gasLimit: 3000000
        });
        const proxy = await deployContract(wallet, proxyCode, [contract.address, initArgsInBytes], {
            gasLimit: 3000000
        });

        const returnContract = new ethers.Contract(proxy.address, contractCode.abi, wallet);
        return [returnContract, contract.address];
    } catch (err) {
        console.log('Error deploying proxy contract: ', err);
    }
}

async function getCallRevertReason(f) {
    let revertReason = DEFAULT_REVERT_REASON;
    let result;
    try {
        result = await f();
    } catch (e) {
        try {
            const data = e.stackTrace[e.stackTrace.length - 1].message.slice(4);
            revertReason = ethers.utils.defaultAbiCoder.decode(['string'], data)[0];
        } catch (err2) {
            throw e;
        }
        // revertReason = (e.reason && e.reason[0]) || e.results[e.hashes[0]].reason
    }
    return { revertReason, result };
}

module.exports = {
    provider,
    wallet,
    wallet1,
    wallet2,
    exitWallet,
    deployTestContract,
    deployProxyContract,
    getCallRevertReason,
    IERC20_INTERFACE,
    DEFAULT_REVERT_REASON
};
