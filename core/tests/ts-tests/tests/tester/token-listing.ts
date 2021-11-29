import { Tester } from './tester';
import { expect } from 'chai';
import * as zksync from 'zksync';
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
        require(`${process.env['ZKSYNC_HOME']}/contracts/artifacts/cache/solpp-generated-contracts/dev-contracts/TestnetERC20Token.sol/TestnetERC20Token.json`).bytecode;
    const tokenAbi = ['constructor(string memory name, string memory symbol, uint8 decimals)'];
    const factory = new ContractFactory(tokenAbi, bytecode, this.ethWallet);
    const tokenContract = await factory.deploy('Test Token', 'TTT', 18);
    await tokenContract.deployTransaction.wait();

    await this.submitToken(tokenContract.address);

    // Waiting for server process the token add event
    const MAX_WAIT = maxWaitForTokenAdditionMs();
    const RETRY_INTERVAL = 100; // 100ms between attempts.
    let contains = false;
    for (let i = 0; i < MAX_WAIT / RETRY_INTERVAL; i++) {
        const tokens = await this.syncProvider.getTokens();

        for (const symbol in tokens) {
            if (tokens[symbol].address === tokenContract.address.toLowerCase()) {
                contains = true;
                break;
            }
        }
        if (contains) {
            break;
        }
        await zksync.utils.sleep(RETRY_INTERVAL);
    }
    expect(contains).to.eql(true);
};

Tester.prototype.testNonERC20Listing = async function () {
    // Non-ERC20 contract
    const bytecode =
        require(`${process.env['ZKSYNC_HOME']}/contracts/artifacts/cache/solpp-generated-contracts/Config.sol/Config.json`).bytecode;
    const factory = new ContractFactory([], bytecode, this.ethWallet);
    const tokenContract = await factory.deploy();
    await tokenContract.deployTransaction.wait();

    await this.submitToken(tokenContract.address);

    // Waiting for server process the token add event.
    // We use the same interval as in test that checks that we *can* process the token given that time.
    // This interval is greater than the interval server uses internally for processing, so if token
    // is not processed, it is *likely* that it will not be processed at all.
    await zksync.utils.sleep(maxWaitForTokenAdditionMs());

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

function maxWaitForTokenAdditionMs(): number {
    if (!process.env.TOKEN_HANDLER_POLL_INTERVAL) {
        throw new Error('TOKEN_HANDLER_POLL_INTERVAL env is not defined');
    }

    // Server must process the event within two processing intervals that it uses.
    const doubleServerIntervalSecs = 2 * parseInt(process.env.TOKEN_HANDLER_POLL_INTERVAL);
    return doubleServerIntervalSecs * 1000;
}
