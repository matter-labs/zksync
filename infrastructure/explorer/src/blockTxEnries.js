import { shortenHash, formatDate, makeEntry, formatToken } from './utils';
import { blockchainExplorerAddress } from './constants';

import {
    getFromAddressOfTx,
    getTxFromFallbackValue,
    getTxToAddress,
    getTxToFallbackValue,
    getTxToken,
    getTxAmount,
    getTxFee,
    numOrZero
} from './blockUtils';

function getTxHashEntry(tx) {
    const entry = makeEntry('Tx Hash');
    entry.localLink(`/transactions/${tx.tx_hash}`);

    entry.innerHTML(shortenHash(tx.tx_hash));
    return entry;
}

function getTxTypeEntry(tx) {
    return makeEntry('Type').innerHTML(tx.op.type);
}

function getTxFromEntry(tx) {
    const entry = makeEntry('From');

    const fromAddress = getFromAddressOfTx(tx);
    const fallback = getTxFromFallbackValue(tx);

    if (tx.op.type === 'Deposit') {
        entry.outterLink(`${blockchainExplorerAddress}/${fromAddress}`);
    } else {
        entry.localLink(`/accounts/${fromAddress}`);
    }

    entry.innerHTML(shortenHash(fromAddress, fallback));
    return entry;
}

function getTxToEntry(tx) {
    const entry = makeEntry('To');

    if (tx.op.type === 'ChangePubKey') {
        return entry;
    }

    const toAddress = getTxToAddress(tx);
    const fallback = getTxToFallbackValue(tx);

    const onChainWithdrawals = ['Withdraw', 'ForcedExit', 'FullExit'];

    if (onChainWithdrawals.includes(tx.op.type)) {
        entry.outterLink(`${blockchainExplorerAddress}/${toAddress}`);
    } else {
        entry.localLink(`/accounts/${toAddress}`);
    }

    entry.innerHTML(shortenHash(toAddress, fallback));

    return entry;
}

async function getTxAmountEntry(tx, token, client) {
    const entry = makeEntry('Amount');
    if (tx.op.type === 'ChangePubKey') {
        return entry;
    }

    const amount = await getTxAmount(tx, client);
    return entry.innerHTML(`${formatToken(numOrZero(amount), token)} ${token}`);
}

function getTxFeeEntry(tx, token) {
    const entry = makeEntry('Fee');
    const fee = getTxFee(tx);

    if (!fee && tx.op.type != 'ChangePubKey') {
        return entry;
    }

    return entry.innerHTML(`${formatToken(numOrZero(fee), token)} ${token}`);
}

function getTxCreatedAtEntry(tx) {
    return makeEntry('Created at').innerHTML(formatDate(tx.created_at));
}

export async function getTxEntries(tx, tokens, client) {
    const tokenSymbol = tokens[getTxToken(tx)].syncSymbol;

    const txHash = getTxHashEntry(tx);
    const type = getTxTypeEntry(tx);
    const from = getTxFromEntry(tx);
    const to = getTxToEntry(tx);
    const amount = await getTxAmountEntry(tx, tokenSymbol, client);
    const fee = getTxFeeEntry(tx, tokenSymbol);
    const createdAt = getTxCreatedAtEntry(tx);

    return {
        txHash,
        type,
        from,
        to,
        amount,
        fee,
        createdAt
    };
}
