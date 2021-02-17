import { Tester } from './tester';
import { expect } from 'chai';
import fs from 'fs';
import fetch from 'node-fetch';
import { Wallet, types, utils } from 'zksync';
import { BigNumber, ethers } from 'ethers';
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
        testForcedExitRequestOneToken(
            from: Wallet,
            to: ethers.Signer,
            token: TokenLike,
            value: BigNumber
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

Tester.prototype.testForcedExitRequestOneToken = async function (
    from: Wallet,
    to: ethers.Signer,
    token: TokenLike,
    amount: BigNumber
) {
    const toAddress = await to.getAddress();
    const tokenAddress = await this.syncProvider.tokenSet.resolveTokenAddress(token);
    let toBalanceBefore = await utils.getEthereumBalance(this.ethProvider, this.syncProvider, toAddress, token);

    const transferHandle = await from.syncTransfer({
        to: toAddress,
        token,
        amount
    });
    await transferHandle.awaitReceipt();

    const status = await getStatus();

    expect(status.status).to.eq('enabled', 'Forced exit requests status is disabled');

    const tokenId = await this.syncProvider.tokenSet.resolveTokenId(token);
    const request = await submitRequest(toAddress, [tokenId], status.request_fee);

    const contractAddress = status.forced_exit_contract_address;

    const amountToPay = BigNumber.from(request.priceInWei).add(BigNumber.from(request.id));

    const gasPrice = (await to.provider?.getGasPrice()) as BigNumber;

    const txHandle = await to.sendTransaction({
        value: amountToPay,
        gasPrice: gasPrice,
        to: contractAddress
    });

    const receipt = await txHandle.wait();

    // We have to wait for verification and execution of the
    // block with the forced exit, so waiting for a while is fine
    let timeout = 45000;
    let interval = 500;

    let timePassed = 0;

    let spentOnGas = receipt.gasUsed.mul(gasPrice);
    let spentTotal = spentOnGas.add(amountToPay);

    let expectedToBalance = toBalanceBefore.add(amount).sub(spentTotal);
    while (timePassed <= timeout) {
        let balance = await getFullOnchainBalance(this, toAddress, tokenAddress);

        if (balance.eq(expectedToBalance)) {
            break;
        }

        await sleep(interval);
        timePassed += interval;
    }

    let balance = await getFullOnchainBalance(this, toAddress, tokenAddress);

    expect(balance.eq(expectedToBalance), 'The ForcedExit has not completed').to.be.true;
};
