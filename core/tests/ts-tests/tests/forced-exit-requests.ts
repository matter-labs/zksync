import { Tester } from './tester';
import { expect } from 'chai';
import fs from 'fs';
import fetch from 'node-fetch';
import { Wallet, types, utils, wallet } from 'zksync';
import { BigNumber, BigNumberish, ethers } from 'ethers';
import * as path from 'path';

import { Address } from 'zksync/build/types';
import { sleep } from 'zksync/build/utils';

import './transfer';

const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const apiTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/api.json`, { encoding: 'utf-8' }));
const apiUrl = `${apiTestConfig.rest_api_url}/api/forced_exit_requests/v0.1`;

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testForcedExitRequestMultipleTokens(
            from: Wallet,
            payer: ethers.Signer,
            to: Address,
            tokens: TokenLike[],
            value: BigNumber[]
        ): Promise<void>;
    }
}

interface StatusResponse {
    status: 'enabled' | 'disabled';
    request_fee: string;
    max_tokens_per_request: number;
    recomended_tx_interval_millis: number;
    forced_exit_contract_address: Address;
}

Tester.prototype.testForcedExitRequestMultipleTokens = async function (
    from: Wallet,
    payer: ethers.Signer,
    to: Address,
    tokens: TokenLike[],
    amounts: BigNumber[]
) {
    const tokenAddresses = tokens.map((token) => this.syncProvider.tokenSet.resolveTokenAddress(token));

    const toBalancesBeforePromises = tokens.map((token, i) => {
        return getFullOnchainBalance(this, to, tokenAddresses[i]);
    });

    let toBalancesBefore = await Promise.all(toBalancesBeforePromises);

    const batchBuilder = from.batchBuilder();
    tokens.forEach((token, i) => {
        batchBuilder.addTransfer({
            to,
            token,
            amount: amounts[i]
        });
    });
    const batch = await batchBuilder.build('ETH');
    const handles = await wallet.submitSignedTransactionsBatch(from.provider, batch.txs, [batch.signature]);

    // Waiting only for the first tx since we send the transactions in batch
    await handles[0].awaitReceipt();

    const status = await getStatus();

    expect(status.status).to.eq('enabled', 'Forced exit requests status is disabled');

    const tokenIds = tokens.map((token) => this.syncProvider.tokenSet.resolveTokenId(token));

    const requestPrice = BigNumber.from(status.request_fee).mul(tokens.length);
    const request = await submitRequest(to, tokenIds, requestPrice.toString());

    const contractAddress = status.forced_exit_contract_address;

    const amountToPay = requestPrice.add(BigNumber.from(request.id));

    const gasPrice = (await payer.provider?.getGasPrice()) as BigNumberish;

    const txHandle = await payer.sendTransaction({
        value: amountToPay,
        gasPrice: gasPrice,
        to: contractAddress
    });

    await txHandle.wait();

    // We have to wait for verification and execution of the
    // block with the forced exit, so waiting for a while is fine
    let timeout = 120000;
    let interval = 500;

    let timePassed = 0;

    let expectedToBalance = toBalancesBefore.map((balance, i) => balance.add(amounts[i]));
    while (timePassed <= timeout) {
        const balancesPromises = tokenAddresses.map((address) => getFullOnchainBalance(this, to, address));
        const balances = await Promise.all(balancesPromises);

        const allExpected = balances.every((bal, i) => bal.eq(expectedToBalance[i]));

        if (allExpected) {
            break;
        }

        await sleep(interval);
        timePassed += interval;
    }

    const balancesPromises = tokenAddresses.map((address) => getFullOnchainBalance(this, to, address));
    const balances = await Promise.all(balancesPromises);
    const allExpected = balances.every((bal, i) => bal.eq(expectedToBalance[i]));

    expect(allExpected, 'The ForcedExit has not completed').to.be.true;
};

async function getStatus() {
    const endpoint = `${apiUrl}/status`;

    const response = await fetch(endpoint);

    return (await response.json()) as StatusResponse;
}

async function submitRequest(address: string, tokens: number[], price_in_wei: string) {
    const endpoint = `${apiUrl}/submit`;

    const data = {
        target: address,
        tokens,
        price_in_wei
    };

    const response = await fetch(endpoint, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        redirect: 'follow',
        body: JSON.stringify(data)
    });

    return await response.json();
}

async function getFullOnchainBalance(tester: Tester, address: Address, tokenAddress: Address) {
    const onchainBalance = await utils.getEthereumBalance(
        tester.ethProvider,
        tester.syncProvider,
        address,
        tokenAddress
    );
    const pendingToBeOnchain = await tester.contract.getPendingBalance(address, tokenAddress);

    return BigNumber.from(onchainBalance).add(BigNumber.from(pendingToBeOnchain));
}
