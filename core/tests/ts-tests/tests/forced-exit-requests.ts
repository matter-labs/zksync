import { Tester } from './tester';
import { expect } from 'chai';
import { Wallet, types, Provider, utils } from 'zksync';
import { BigNumber, ethers } from 'ethers';
import { Address } from 'zksync/build/types';

import * as path from 'path';
import fs from 'fs';
import { RevertReceiveAccountFactory, RevertTransferERC20Factory } from '../../../../contracts/typechain';
import { waitForOnchainWithdrawal, loadTestConfig } from './helpers';

import fetch from 'node-fetch';

import './transfer';
import { reporters } from 'mocha';

const TEST_CONFIG = loadTestConfig();

const apiTypesFolder = './api-types';
const ADDRESS_REGEX = /^0x([0-9a-fA-F]){40}$/;
const DATE_REGEX = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?/;
const testConfigPath = path.join(process.env.ZKSYNC_HOME as string, `etc/test_config/constant`);
const apiTestConfig = JSON.parse(fs.readFileSync(`${testConfigPath}/api.json`, { encoding: 'utf-8' }));
const apiUrl = `${apiTestConfig.rest_api_url}/api/forced_exit_requests/v0.1`;

type TokenLike = types.TokenLike;

declare module './tester' {
    interface Tester {
        testForcedExitRequestOneToken(from: Wallet, to: ethers.Signer, token: TokenLike, value: BigNumber): Promise<void>;
    }
}

interface StatusResponse {
    status: "enabled" | "disabled",
    request_fee: string,
    max_tokens_per_request: number,
    recomended_tx_interval_millis: number
}

async function getStatus() {
    const endpoint = `${apiUrl}/status`;
    
    const response = await fetch(endpoint);

    console.log(response.status);
   /// console.log(await response.text());

    return (await response.json()) as StatusResponse;
} 

async function submitRequest(
    address: string,
    tokens: number[],
    price_in_wei: string
) {
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

Tester.prototype.testForcedExitRequestOneToken = async function (
    from: Wallet,
    to: ethers.Signer,
    token: TokenLike,
    amount: BigNumber
) {
    const toAddress = await to.getAddress();
    let toBalanceBefore = await utils.getEthereumBalance(
        this.ethProvider,
        this.syncProvider,
        toAddress,
        token
    );

    const transferHandle = await from.syncTransfer({
        to: toAddress,
        token,
        amount,
    });

    await transferHandle.awaitReceipt();

    const status = await getStatus();

    const tokenId = await this.syncProvider.tokenSet.resolveTokenId(token);

    console.log(await submitRequest(
        toAddress,
        [tokenId],
        status.request_fee
    ));
   //  expect(toBalance.eq(expectedToBalance), 'The withdrawal was not recovered').to.be.true;
};

