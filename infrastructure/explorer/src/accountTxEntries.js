import { shortenHash, formatDate, makeEntry } from './utils';
import { blockchainExplorerAddress } from './constants';

function getHashEntry(tx) {
    if (tx.hash.startsWith('sync-tx:')) {
        tx.hash = tx.hash.slice('sync-tx:'.length);
    }

    return makeEntry('TxHash')
        .localLink(`/transactions/${tx.hash}`)
        .innerHTML(`${shortenHash(tx.hash, 'unknown! hash')}`);
}

function getLinkFromEntry(tx) {
    const entry = makeEntry('From');

    if (tx.type == 'Deposit') {
        entry.outterLink(`${blockchainExplorerAddress}/${tx.from}`);
    } else {
        entry.localLink(`/accounts/${tx.from}`);
    }

    return entry.innerHTML(`${shortenHash(tx.from, 'unknown! from')}`);
}

function getLinkToEntry(tx) {
    const entry = makeEntry('To');

    if (tx.type == 'ChangePubKey') {
        return entry;
    }

    if (tx.type == 'Withdrawal') {
        entry.outterLink(`${blockchainExplorerAddress}/${tx.to}`);
    } else {
        entry.localLink(`/accounts/${tx.to}`);
    }

    return entry.innerHTML(`${shortenHash(tx.to, 'unknown! to')}`);
}

function getTypeEntry(tx) {
    return makeEntry('Type').innerHTML(tx.type);
}

function getAmountEntry(tx) {
    const entry = makeEntry('Amount');
    if (tx.type === 'ChangePubKey') {
        return entry;
    }

    return entry.innerHTML(`${tx.token} <span>${tx.amount}</span>`);
}

function getCreatedAtEntry(tx) {
    return makeEntry('CreatedAt').innerHTML(formatDate(tx.created_at));
}

export function getTxEntries(tx) {
    return {
        TxHash: getHashEntry(tx),
        Type: getTypeEntry(tx),
        Amount: getAmountEntry(tx),
        From: getLinkFromEntry(tx),
        To: getLinkToEntry(tx),
        CreatedAt: getCreatedAtEntry(tx)
    };
}
