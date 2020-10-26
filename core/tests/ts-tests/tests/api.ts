import fs from 'fs';
import fetch from 'node-fetch';
import { expect } from 'chai';

import { Interface as StatusInterface } from '../api-types/status';
import { Interface as BlockInterface } from '../api-types/block';
import { Interface as BlocksInterface } from '../api-types/blocks';
import { Interface as TxHistoryInterface } from '../api-types/tx-history';
import { Interface as TestnetConfigInterface } from '../api-types/config';
import { Interface as BlockTransactionsInterface } from '../api-types/block-transactions';
import { Interface as TransactionInterface } from '../api-types/transaction';

const apiTypesFolder = './api-types';
const ADDRESS_REGEX = /^0x([0-9a-fA-F]){40}$/;
const DATE_REGEX = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?/;

// Checks that json string has the expected js type.
// Usage: pass a path to .ts file that exports a type named `Interface` and a json string.
// A new .gen.ts file will be generated, with your json string assigned to an `Interface`-typed variable.
// If this file compiles, the types match.
// If it doesn't, see the generated file to find what fields don't match.
async function validateTypeJSON(typeFilePath: string, json: string) {
    const tmpFilePath = typeFilePath.replace(
        /\.ts$/,
        `${new Date().toString().slice(16, 24).replace(/\:/g, '_')}.gen.ts`
    );
    const typeContent = fs.readFileSync(typeFilePath, 'utf-8');
    fs.writeFileSync(tmpFilePath, `${typeContent}\n\nexport const val: Interface = ${json};\n`);

    try {
        require('../' + tmpFilePath);
        fs.unlinkSync(tmpFilePath);
    } catch (e) {
        expect.fail(`Rest api response format error in type ${typeFilePath}:\n${json}`);
    }
}

// A helper function that fetches data from url (currently supports only GET requests)
// and calls `validateTypeJSON` for response type checking.
// If `validateTypeJSON` doesn't find any errors, returns the fetched data.
async function validateResponseFromUrl(typeFilePath: string, url: string): Promise<any> {
    const response = await fetch(url).catch(() => {
        throw new Error(`Request to ${url} failed with status code ${response.status}`);
    });
    const data = await response.json();
    const serverJson = JSON.stringify(data, null, 4);
    await validateTypeJSON(typeFilePath, serverJson);
    return data;
}

export async function checkStatusResponseType(): Promise<StatusInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/status`;
    const typeFilePath = `${apiTypesFolder}/status.ts`;
    const data: StatusInterface = await validateResponseFromUrl(typeFilePath, url);
    return data;
}

export async function checkWithdrawalProcessingTimeResponseType(): Promise<StatusInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/withdrawal_processing_time`;
    const typeFilePath = `${apiTypesFolder}/withdrawal-processing.ts`;
    const data: StatusInterface = await validateResponseFromUrl(typeFilePath, url);
    return data;
}

export async function checkTxHistoryResponseType(address: string): Promise<TxHistoryInterface> {
    const offset = 0;
    const limit = 20;
    const url = `${process.env.REST_API_ADDR}/api/v0.1/account/${address}/history/${offset}/${limit}`;
    const typeFilePath = `${apiTypesFolder}/tx-history.ts`;
    const data: TxHistoryInterface = await validateResponseFromUrl(typeFilePath, url);

    for (const tx of data) {
        expect(tx.created_at, 'Wrong date format').to.match(DATE_REGEX);
    }

    return data;
}

export async function checkBlockResponseType(blockNumber: number): Promise<BlockInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}`;
    const typeFilePath = `${apiTypesFolder}/block.ts`;
    const data: BlockInterface = await validateResponseFromUrl(typeFilePath, url);
    expect(data.committed_at, 'Wrong date format').to.match(DATE_REGEX);
    return data;
}

export async function checkBlocksResponseType(): Promise<BlocksInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks`;
    const typeFilePath = `${apiTypesFolder}/blocks.ts`;
    const data: BlocksInterface = await validateResponseFromUrl(typeFilePath, url);
    return data;
}

export async function checkBlockTransactionsResponseType(blockNumber: number): Promise<BlockTransactionsInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}/transactions`;
    const typeFilePath = `${apiTypesFolder}/block-transactions.ts`;
    const data: BlockTransactionsInterface = await validateResponseFromUrl(typeFilePath, url);

    for (const tx of data) {
        expect(tx.created_at, 'Wrong date format').to.match(DATE_REGEX);
    }

    return data;
}

export async function checkTestnetConfigResponseType(): Promise<TestnetConfigInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/testnet_config`;
    const typeFilePath = `${apiTypesFolder}/config.ts`;
    const data: TestnetConfigInterface = await validateResponseFromUrl(typeFilePath, url);
    expect(data.contractAddress, 'Wrong address format').to.match(ADDRESS_REGEX);
    return data;
}

export async function checkTransactionsResponseType(txHash: string): Promise<TransactionInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/transactions_all/${txHash}`;
    const typeFilePath = `${apiTypesFolder}/transaction.ts`;
    const data: TransactionInterface = await validateResponseFromUrl(typeFilePath, url);
    expect(data.created_at, 'Wrong date format').to.match(DATE_REGEX);
    return data;
}

export function deleteUnusedGenFiles() {
    fs.readdirSync(apiTypesFolder)
        .filter((n) => n.endsWith('.gen.ts'))
        .map((n) => apiTypesFolder + '/' + n)
        .forEach(fs.unlinkSync);
}
