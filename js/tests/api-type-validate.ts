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
        console.error(`Check file ${tmpFilePath} to see the error`)

        throw new Error(`Rest api response format error.`);
    }
}

async function validateResponseFromUrl(typeFilePath: string, url: string) {
    const { data } = await Axios.get(url);
    const serverJson = JSON.stringify(data, null, 4);

    await validateTypeJSON(typeFilePath, serverJson);

    return data;
}

function assertAddress(address: string) {
    if (! /^0x([0-9a-fA-F]){40}$/.test(address)) {
        throw new Error(address + ' is not address!');
    }
}

function assertPubKey(pubKey: string) {
    if (! /^sync:([0-9a-fA-F]){40}$/.test(pubKey)) {
        throw new Error(pubKey + ' is not pubkey!');
    }
}

function assertNumeric(number: string) {
    if (! /^\d+$/.test(number)) {
        throw new Error(number + ' is not numeric!');
    }
}

function assertHash(hash: string) {
    if (! /^0x([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is not a hash!');
    }
}

function assertSyncHash(hash: string) {
    if (! /^sync-tx:([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is not a sync hash!');
    }
}

function assertEthOrSyncHash(hash: string) {
    if (! /^(sync-tx:|0x)([0-9a-fA-F]){64}$/.test(hash)) {
        throw new Error(hash + ' is neither a sync nor eth hash!');
    }
}

function assertDate(date: string) {
    // 2020-04-29T11:14:03.198603
    return /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}/.test(date);
}

export async function checkStatus(): Promise<StatusInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/status`;
    const typeFilePath = `${apiTypesFolder}/status.ts`;
    const data: StatusInterface = await validateResponseFromUrl(typeFilePath, url);

    return data;
}

export async function checkAccount(address: string): Promise<AccountInterface> {
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

export async function checkTxHistory(address: string): Promise<TxHistoryInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/account/${address}/history/0/20`;
    const typeFilePath = `${apiTypesFolder}/tx-history.ts`;
    const data: TxHistoryInterface = await validateResponseFromUrl(typeFilePath, url);

    for (const tx of data) {
        assertDate(tx.created_at);
    }

    return data;
}

export async function checkBlock(blockNumber: number): Promise<BlockInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}`;
    const typeFilePath = `${apiTypesFolder}/block.ts`;
    const data: BlockInterface = await validateResponseFromUrl(typeFilePath, url);

    assertDate(data.committed_at);

    return data;
}

export async function checkBlocks(): Promise<BlocksInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks`;
    const typeFilePath = `${apiTypesFolder}/blocks.ts`;
    const data: BlocksInterface = await validateResponseFromUrl(typeFilePath, url);

    return data;
}


export async function checkBlockTransactions(blockNumber: number): Promise<BlockTransactionsInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/blocks/${blockNumber}/transactions`;
    const typeFilePath = `${apiTypesFolder}/block-transactions.ts`;
    const data: BlockTransactionsInterface = await validateResponseFromUrl(typeFilePath, url);

    for (const tx of data) {
        assertDate(tx.created_at);
    }

    return data;
}

export async function checkTestnetConfig(): Promise<TestnetConfigInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/testnet_config`;
    const typeFilePath = `${apiTypesFolder}/config.ts`;
    const data: TestnetConfigInterface = await validateResponseFromUrl(typeFilePath, url);

    assertAddress(data.contractAddress);

    return data;
}

export async function checkTransactions(txHash: string): Promise<TransactionInterface> {
    const url = `${process.env.REST_API_ADDR}/api/v0.1/transactions_all/${txHash}`;
    const typeFilePath = `${apiTypesFolder}/transaction.ts`;
    const data: TransactionInterface = await validateResponseFromUrl(typeFilePath, url);

    assertDate(data.created_at);

    return data;
}

export function deleteUnusedGenFiles() {
    fs.readdirSync(apiTypesFolder)
        .filter(n => n.endsWith('.gen.ts'))
        .map(n => apiTypesFolder + '/' + n)
        .forEach(fs.unlinkSync);
}

let syncProvider: zksync.Provider;
let ethersProvider: ethers.providers.Provider;

async function test() {
    syncProvider = await zksync.Provider.newWebsocketProvider(process.env.WS_API_ADDR);
    const ethersProvider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    
    const ethWallet = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/1"
    ).connect(ethersProvider);
    
    const ethWallet2 = ethers.Wallet.fromMnemonic(
        process.env.MNEMONIC,
        "m/44'/60'/0'/0/2"
    ).connect(ethersProvider);

    const syncWallet = await zksync.Wallet.fromEthSigner(ethWallet, syncProvider);

    for (const token of ['ETH', process.env.TEST_ERC20]) {
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
    await checkStatus();
    await checkTestnetConfig();

    console.log("Checking account and tx history");
    await checkAccount(syncWallet.address());
    await checkTxHistory(syncWallet.address());
    
    const numBlocksToCheck = 10;

    const blocks = await checkBlocks();
    for (const { block_number } of blocks.slice(-numBlocksToCheck)) {
        console.log(`Checking block ${block_number}`);
        await checkBlock(block_number);
        const txs = await checkBlockTransactions(block_number);
        for (const { tx_hash } of txs) {
            await checkTransactions(tx_hash);
        }
    }

    deleteUnusedGenFiles();

    await syncProvider.disconnect();
}

test().catch(e => {
    console.error(e);
    process.exit(1);
});
