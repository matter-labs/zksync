const ethers = require('ethers');
const { use } = require('chai');
const { solidity, deployContract } = require('ethereum-waffle');

const path = require('path');
const fs = require('fs');

const IERC20_INTERFACE = require('@openzeppelin/contracts/build/contracts/IERC20');
const { rawEncode } = require('ethereumjs-abi');
const DEFAULT_REVERT_REASON = 'VM did not revert';
const testConfigPath = path.join(process.env.ZKSYNC_HOME, `etc/test_config/constant`);
const ethTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/eth.json`, { encoding: 'utf-8' }));

// For: geth

// const provider = new ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL);
// const wallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
// const exitWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/2").connect(provider);

// For: ganache
//const provider = new MockProvider({ ganacheOptions: { gasLimit: '8000000', gasPrice: '1' } });
//const [wallet, wallet1, wallet2, exitWallet] = provider.getWallets();

//For: rskj
const provider = new ethers.providers.JsonRpcProvider(process.env.ETH_CLIENT_WEB3_URL);
const wallet = new ethers.Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow_privK, 'hex'), provider);
const wallet1 = new ethers.Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow1_privK, 'hex'), provider);
const wallet2 = new ethers.Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow2_privK, 'hex'), provider);
const exitWallet = new ethers.Wallet(Buffer.from(ethTestConfig.account_with_rbtc_cow3_privK, 'hex'), provider);

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
        /*try {
            const data = e.stackTrace[e.stackTrace.length - 1].message.value.slice(4);
            revertReason = ethers.utils.defaultAbiCoder.decode(['string'], data)[0];
        } catch (err2) {
            throw e;
        }*/
        const message = e.toString();
        let idx = message.lastIndexOf('revert');
        if (idx > 0) {
            revertReason = message.substring(idx + 6, message.length).trim();
        } else {
            revertReason = message;
        }
    }
    return { revertReason, result };
}

async function evmMine() {
    return provider.send('evm_mine', []);
}

async function evmMineMany(count) {
    for (let i = 0; i < count; i++) {
        await evmMine();
    }
}

async function increaseTime(time) {
    try {
        const result = await provider.send('evm_increaseTime', [time]);
        await evmMine();
        return result;
    } catch (error) {
        return null;
    }
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
    DEFAULT_REVERT_REASON,
    evmMine,
    evmMineMany,
    increaseTime
};
