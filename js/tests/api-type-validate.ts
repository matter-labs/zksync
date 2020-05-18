import * as fs from 'fs';
import Axios from 'axios';
import * as assert from 'assert';

import { Interface as StatusInterface } from './api-types/status';
import { Interface as BlockInterface } from './api-types/block';
import { Interface as BlocksInterface } from './api-types/blocks';
import { Interface as AccountInterface } from './api-types/account';
import { Interface as TxHistoryInterface } from './api-types/tx-history';
import { Interface as TestnetConfigInterface } from './api-types/config';
import { Interface as BlockTransactionsInterface } from './api-types/block-transactions';
import { Interface as TransactionInterface } from './api-types/transaction';

import * as zksync from 'zksync';
import * as ethers from 'ethers';
import { bigNumberify, parseEther, formatEther } from 'ethers/utils';


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

    fs.writeFileSync(
        tmpFilePath,
        typeContent + `\n\nexport const val: Interface = ` + json + ';\n'
    );

    try {
        require(tmpFilePath);
        fs.unlinkSync(tmpFilePath);
    } catch (e) {
        console.error(`Error in type ${typeFilePath}:`)
        console.error(e.message.split('\n').slice(2, 7).join('\n'));
        console.error(`Check file ${tmpFilePath} to see the error.`);
        console.error(`Edit ${typeFilePath} to match the new format,`);
        console.error(`and don't forget to check that frontend (e.g. explorer) work!`);

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
    const { data } = await Axios.get(url)
        .catch(e => {
            throw new Error(`Request to ${e.config.url} failed with status code ${e.response.status}`);
        });

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
    if (! /^0x([0-9a-fA-F]){40}$/.test(address)) {
        throw new Error(address + ' is not address!');
    }
}

/**
 * Checks that string is a zkSync pubkey (starts with `sync:`, followed by 40 hex chars)
 * @param pubKey: string to check
 */
function assertPubKey(pubKey: string) {
    if (! /^sync:([0-9a-fA-F]){40}$/.test(pubKey)) {
        throw new Error(pubKey + ' is not pubkey!');
    }
}

/**
 * Checks that string can be parsed as integer.
 * @param number: string to check
 */
function assertNumeric(number: string) {
    if (! (/^\d+$/.test(number) && (number == '0' || number[0] != '0'))) {
        throw new Error(number + ' is not numeric!');
    }
}

/**
 * Checks that string is an eth hash (starts with `0x`, followed by 64 hex characters)
 * @param hash: string to check
 */
function assertHash(hash: string) {
    if (! /^0x([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is not a hash!');
    }
}

/**
 * Checks that string is a sync hash (starts with `sync-tx:`, followed by 64 hex characters)
 * @param hash: string to check
 */
function assertSyncHash(hash: string) {
    if (! /^sync-tx:([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is not a sync hash!');
    }
}

/**
 * Checks that string is either sync or eth hash.
 * @param hash: string to check
 */
function assertEthOrSyncHash(hash: string) {
    if (! /^(sync-tx:|0x)([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is neither a sync nor eth hash!');
    }
}

/**
 * Checks that string date representation looks like 2020-04-29T11:14:03.198603,
 * which is the format currently used in block explorer.
 * @param date: string to check
 */
function assertDate(date: string) {
    if (! /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}/.test(date)) {
        throw new Error(date + `doesn't conform to 2020-04-29T11:14:03.198603 format.`);
    }
}

/**
 * Check `/status` method of our rest api
 */
export async function checkStatusResponseType(): Promise<StatusInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/status`;
    const typeFilePath = `${apiTypesFolder}/status.ts`;
    const data: StatusInterface = await validateResponseFromUrl(typeFilePath, url);

    return data;
}

/**
 * Check `/account/${address}` method of our rest api
 */
export async function checkAccountInfoResponseType(address: string): Promise<AccountInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/account/${address}`;
    const typeFilePath = `${apiTypesFolder}/account.ts`;
    const data: AccountInterface = await validateResponseFromUrl(typeFilePath, url);

    // additional checks
    assertPubKey(data.commited.pub_key_hash);
    assertAddress(data.commited.address);
    Object.values(data.commited.balances).forEach(assertNumeric);

    assertPubKey(data.verified.pub_key_hash);
    assertAddress(data.verified.address);
    Object.values(data.verified.balances).forEach(assertNumeric);

    return data;
}

/**
 * Check `/account/${address}/history/${offset}/${limit}` method of our rest api
 */
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

/**
 * Check `/blocks/${blockNumber}` method of our rest api
 */
export async function checkBlockResponseType(blockNumber: number): Promise<BlockInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}`;
    const typeFilePath = `${apiTypesFolder}/block.ts`;
    const data: BlockInterface = await validateResponseFromUrl(typeFilePath, url);

    assertDate(data.committed_at);

    return data;
}

/**
 * Check `/blocks` method of our rest api
 */
export async function checkBlocksResponseType(): Promise<BlocksInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks`;
    const typeFilePath = `${apiTypesFolder}/blocks.ts`;
    const data: BlocksInterface = await validateResponseFromUrl(typeFilePath, url);

    return data;
}

/**
 * Check `/blocks/${blockNumber}/transactions` method of our rest api
 */
export async function checkBlockTransactionsResponseType(blockNumber: number): Promise<BlockTransactionsInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}/transactions`;
    const typeFilePath = `${apiTypesFolder}/block-transactions.ts`;
    const data: BlockTransactionsInterface = await validateResponseFromUrl(typeFilePath, url);

    for (const tx of data) {
        assertDate(tx.created_at);
    }

    return data;
}
/**
 * Check `/testnet_config` method of our rest api
 */
export async function checkTestnetConfigResponseType(): Promise<TestnetConfigInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/testnet_config`;
    const typeFilePath = `${apiTypesFolder}/config.ts`;
    const data: TestnetConfigInterface = await validateResponseFromUrl(typeFilePath, url);

    assertAddress(data.contractAddress);

    return data;
}
/**
 * Check `/transactions_all/${txHash}` method of our rest api
 */
export async function checkTransactionsResponseType(txHash: string): Promise<TransactionInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/transactions_all/${txHash}`;
    const typeFilePath = `${apiTypesFolder}/transaction.ts`;
    const data: TransactionInterface = await validateResponseFromUrl(typeFilePath, url);

    assertDate(data.created_at);

    return data;
}

/**
 * delete all .gen.ts files.
 */
export function deleteUnusedGenFiles() {
    fs.readdirSync(apiTypesFolder)
        .filter(n => n.endsWith('.gen.ts'))
        .map(n => apiTypesFolder + '/' + n)
        .forEach(fs.unlinkSync);
}

let syncProvider: zksync.Provider;
let ethersProvider: ethers.providers.Provider;

/**
 * Performs all transactions available:
 * Deposit,
 * ChangePubKey offchain,
 * Transfer,
 * Withdraw,
 * FullExit
 * 
 * And proceeds to check the type of response of every our rest api method.
 * 
 * Apart from checking the types, this function is handy for testing
 * block explorer, as it performs all transactions, and only once.
 */
async function test() {
    syncProvider = await zksync.Provider.newWebsocketProvider(process.env.WS_API_ADDR);
    ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    
    const ethWallet = ethers.Wallet.fromMnemonic(
        process.env.TEST_MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    
    const ethWallet2 = ethers.Wallet.fromMnemonic(
        process.env.TEST_MNEMONIC,
        "m/44'/60'/0'/0/2"
    ).connect(ethersProvider);

    const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);

    for (const token of ['ETH', "ERC20-1"]) {
        console.log('Balance of ' + token + ': ' + formatEther(await syncWallet.getEthereumBalance(token)));
        const deposit = await syncWallet.depositToSyncFromEthereum({
            depositTo: syncWallet.address(),
            token,
            amount: parseEther("0.01"),
            approveDepositAmountForERC20: true,
        });
        await deposit.awaitReceipt();
        console.log('deposit hash:', deposit.ethTx.hash);

        if (! await syncWallet.isSigningKeySet()) {
            const changePubKey = await syncWallet.setSigningKey();
            await changePubKey.awaitReceipt();
            console.log('changePubKey hash:', changePubKey.txHash);
        }

        const transfer = await syncWallet.syncTransfer({
            to: ethWallet2.address,
            token,
            amount: parseEther("0.002"),
            fee: parseEther('0.0'),
        });
        await transfer.awaitReceipt();
        console.log('transfer hash:', transfer.txHash);


        const withdraw = await syncWallet.withdrawFromSyncToEthereum({
            ethAddress: syncWallet.address(),
            token,
            amount: parseEther("0.002"),
            fee: parseEther('0.0'),
        })
        await withdraw.awaitReceipt();
        console.log('withdraw hash:', withdraw.txHash);


        const fullExit = await syncWallet.emergencyWithdraw({token});
        await fullExit.awaitReceipt();
        console.log('fullExit hash:', fullExit.ethTx.hash);
    }

    deleteUnusedGenFiles();

    console.log("Checking status and testnet config");
    await checkStatusResponseType();
    await checkTestnetConfigResponseType();

    console.log("Checking account and tx history");
    await checkAccountInfoResponseType(syncWallet.address());
    await checkTxHistoryResponseType(syncWallet.address());
    
    const numBlocksToCheck = 10;

    const blocks = await checkBlocksResponseType();
    for (const { block_number } of blocks.slice(-numBlocksToCheck)) {
        console.log(`Checking block ${block_number}`);
        await checkBlockResponseType(block_number);
        const txs = await checkBlockTransactionsResponseType(block_number);
        for (const { tx_hash } of txs) {
            await checkTransactionsResponseType(tx_hash);
        }
    }

    deleteUnusedGenFiles();

    await syncProvider.disconnect();
}

// only with this flag, otherwise it will run even when the file is just imported somewhere.
if (process.argv[2] == '--test') {
    test().catch(e => {
        console.error(e);
        process.exit(1);
    });
}
