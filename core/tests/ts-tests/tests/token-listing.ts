import { Tester } from './tester';
import { expect } from 'chai';
import * as zksync from '@rsksmart/rif-aggregation-sdk-js';
import { ContractFactory } from 'ethers';

declare module './tester' {
    interface Tester {
        testERC20Listing(): Promise<void>;
        testNonERC20Listing(): Promise<void>;
    }
}

Tester.prototype.testERC20Listing = async function () {
    // Simple ERC20 contract
    const bytecode =
        require('../../../../contracts/artifacts/cache/solpp-generated-contracts/dev-contracts/TestnetERC20Token.sol/TestnetERC20Token.json').bytecode;
    const tokenAbi = ['constructor(string memory name, string memory symbol, uint8 decimals)'];
    const factory = new ContractFactory(tokenAbi, bytecode, this.ethWallet);
    const tokenContract = await factory.deploy('Test Token', 'TTT', 18);
    await tokenContract.deployTransaction.wait();

    await this.submitToken(tokenContract.address);

    // Waiting for server process the token add event
    await zksync.utils.sleep(15000);
    const tokens = await this.syncProvider.getTokens();
    let contains = false;
    for (const symbol in tokens) {
        if (tokens[symbol].address === tokenContract.address.toLowerCase()) {
            contains = true;
            break;
        }
    }
    expect(contains).to.eql(true);
};

Tester.prototype.testNonERC20Listing = async function () {
    // Non-ERC20 contract
    const bytecode =
        require('../../../../contracts/artifacts/cache/solpp-generated-contracts/Config.sol/Config.json').bytecode;
    const factory = new ContractFactory([], bytecode, this.ethWallet);
    const tokenContract = await factory.deploy();
    await tokenContract.deployTransaction.wait();

    await this.submitToken(tokenContract.address);

    // Waiting for server process the token add event
    await zksync.utils.sleep(15000);
    const tokens = await this.syncProvider.getTokens();
    let contains = false;
    for (const symbol in tokens) {
        if (tokens[symbol].address === tokenContract.address.toLowerCase()) {
            contains = true;
            break;
        }
    }
    expect(contains).to.eql(false);
};
