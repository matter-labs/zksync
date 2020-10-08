import fs from 'fs';
import fetch from 'node-fetch';

import { Interface as StatusInterface } from './api-types/status';
import { Interface as BlockInterface } from './api-types/block';
import { Interface as BlocksInterface } from './api-types/blocks';
import { Interface as TxHistoryInterface } from './api-types/tx-history';
import { Interface as TestnetConfigInterface } from './api-types/config';
import { Interface as BlockTransactionsInterface } from './api-types/block-transactions';
import { Interface as TransactionInterface } from './api-types/transaction';

const apiTypesFolder = './api-types';

/**
 * Checks that json string has the expected js type.
 *
 * Usage: pass a path to .ts file that exports a type named `Interface` and a json string.
 * A new .gen.ts file will be generated, with your json string assigned to an `Interface`-typed variable.
 * If this file compiles, the types match.
 * If it doesn't, see the generated file to find what fields don't match.
 *
 * @param typeFilePath: the path to .ts file containing expected type of response
 * @param json: A JSON string to check.
 */
async function validateTypeJSON(typeFilePath: string, json: string) {
    const tmpFilePath = typeFilePath.replace(
        /\.ts$/,
        `${new Date().toString().slice(16, 24).replace(/\:/g, '_')}.gen.ts`
    );
    const typeContent = fs.readFileSync(typeFilePath, 'utf-8');

    fs.writeFileSync(tmpFilePath, typeContent + `\n\nexport const val: Interface = ` + json + ';\n');

    try {
        require(tmpFilePath);
        fs.unlinkSync(tmpFilePath);
    } catch (e) {
        console.error(json);
        // console.error(`Error in type ${typeFilePath}:`);
        // console.error(e.message.split('\n').slice(2, 7).join('\n'));
        // console.error(`Check file ${tmpFilePath} to see the error.`);
        // console.error(`Edit ${typeFilePath} to match the new format,`);
        // console.error(`and don't forget to check that frontend (e.g. explorer) work!`);

        throw new Error(`Rest api response format error.`);
    }
}

/**
 * A helper function that fetches data from url (currently supports only GET requests)
 * and calls `validateTypeJSON` for response type checking.
 * If `validateTypeJSON` doesn't find any errors, returns the fetched data.
 * @param typeFilePath: the path to .ts file containing expected type of response
 * @param url: url to fetch data (must contain all the GET parameters needed).
 */
async function validateResponseFromUrl(typeFilePath: string, url: string): Promise<any> {
    const response = await fetch(url).catch(() => {
        throw new Error(`Request to ${url} failed with status code ${response.status}`);
    });
    const data = await response.json();
    const serverJson = JSON.stringify(data, null, 4);

    try {
        await validateTypeJSON(typeFilePath, serverJson);
    } catch (e) {
        console.error(`Error in response type of ${url}`);
        throw e;
    }

    return data;
}

/**
 * Checks that string is a zkSync address (starts with `0x`, followed by 40 hex chars)
 * @param address: string to check
 */
function assertAddress(address: string) {
    if (!/^0x([0-9a-fA-F]){40}$/.test(address)) {
        throw new Error(address + ' is not address!');
    }
}

/**
 * Checks that string date representation looks like 2020-04-29T11:14:03.198603,
 * which is the format currently used in block explorer.
 * @param date: string to check
 */
function assertDate(date: string) {
    if (!/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{6})?/.test(date)) {
        throw new Error(date + `doesn't conform to 2020-04-29T11:14:03.198603 format.`);
    }
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
        assertDate(tx.created_at);
    }

    return data;
}

export async function checkBlockResponseType(blockNumber: number): Promise<BlockInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}`;
    const typeFilePath = `${apiTypesFolder}/block.ts`;
    const data: BlockInterface = await validateResponseFromUrl(typeFilePath, url);
    assertDate(data.committed_at);
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
        assertDate(tx.created_at);
    }

    return data;
}

export async function checkTestnetConfigResponseType(): Promise<TestnetConfigInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/testnet_config`;
    const typeFilePath = `${apiTypesFolder}/config.ts`;
    const data: TestnetConfigInterface = await validateResponseFromUrl(typeFilePath, url);
    assertAddress(data.contractAddress);
    return data;
}

export async function checkTransactionsResponseType(txHash: string): Promise<TransactionInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/transactions_all/${txHash}`;
    const typeFilePath = `${apiTypesFolder}/transaction.ts`;
    const data: TransactionInterface = await validateResponseFromUrl(typeFilePath, url);
    assertDate(data.created_at);
    return data;
}

export function deleteUnusedGenFiles() {
    fs.readdirSync(apiTypesFolder)
        .filter((n) => n.endsWith('.gen.ts'))
        .map((n) => apiTypesFolder + '/' + n)
        .forEach(fs.unlinkSync);
}
