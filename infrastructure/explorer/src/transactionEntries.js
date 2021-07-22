import { formatDate, formatToken, makeEntry, blockchainExplorerToken, readyStateFromString } from './utils';
import { blockchainExplorerAddress } from './constants';
import { BigNumber } from 'ethers';

function fromLinkEntry(txData) {
    const entry = makeEntry('From').copyable();

    if (txData.tx_type == 'Deposit') {
        entry.outterLink(`${blockchainExplorerAddress}/${txData.from}`);
    } else {
        entry.localLink(`/accounts/${txData.from}`);
    }

    if (txData.tx_type == 'Withdrawal' || txData.tx_type == 'FullExit' || txData.tx_type == 'ForcedExit') {
        entry.layer(2);
    }
    if (txData.tx_type == 'Deposit') {
        entry.layer(1);
    }

    if (txData.tx_type == 'ChangePubKey') {
        entry.rename('Account');
    }

    entry.innerHTML(txData.from);

    return entry;
}

function toLinkEntry(txData) {
    const entry = makeEntry('To').copyable();

    if (txData.tx_type == 'Withdrawal') {
        entry.outterLink(blockchainExplorerToken(txData.tokenName, txData.to));
    } else {
        entry.localLink(`/accounts/${txData.to}`);
    }

    if (txData.tx_type == 'Withdrawal' || txData.tx_type == 'FullExit' || txData.tx_type == 'ForcedExit') {
        entry.layer(1);
    }
    if (txData.tx_type == 'Deposit') {
        entry.layer(2);
    }

    return entry.innerHTML(txData.to);
}

function typeEntry(txData) {
    return makeEntry('Type').innerHTML(txData.tx_type);
}

function statusEntry(txData) {
    const entry = makeEntry('Status');

    const statusStr = txData.fail_reason ? 'Rejected' : txData.status;
    entry.innerHTML(statusStr);
    entry.status(readyStateFromString(statusStr));

    return entry;
}

function feeEntry(txData) {
    const fee = txData.fee || 0;
    if (!txData.feeTokenName) {
        return makeEntry('Fee').innerHTML(`${'ETH'} ${formatToken(fee, 'ETH')}`);
    }

    try {
        const feeBN = BigNumber.from(fee);
        if (feeBN.eq('0')) {
            return makeEntry('Fee').innerHTML(
                '<i>This transaction is a part of a batch. The fee was paid in another transaction.</i>'
            );
        }
    } catch {
        return makeEntry('Fee');
    }
    return makeEntry('Fee').innerHTML(`${txData.feeTokenName} ${formatToken(fee, txData.feeTokenName)}`);
}

function createdAtEntry(txData) {
    return makeEntry('Created at').innerHTML(formatDate(txData.created_at));
}

function amountEntry(txData) {
    return makeEntry('Amount').innerHTML(`${txData.tokenName} ${formatToken(txData.amount || 0, txData.feeTokenName)}`);
}

function newSignerPubKeyHashEntry(txData) {
    if (txData.tx_type == 'ChangePubKey') {
        return makeEntry('New signer key hash').innerHTML(`${txData.to.replace('sync:', '')}`);
    } else {
        // This entry won't be used for any tx_type
        // except for ChangePubKey anyway
        return '';
    }
}

export function getTxEntries(txData) {
    const rows = [];

    if (txData.nonce != -1 && (txData.nonce || txData === 0)) {
        rows.push(makeEntry('Nonce').innerHTML(txData.nonce));
    }

    if (txData.numEthConfirmationsToWait) {
        rows.push(makeEntry('Eth confirmations').innerHTML(txData.numEthConfirmationsToWait));
    }

    if (txData.fail_reason) {
        rows.push(makeEntry('Rejection reason:').innerHTML(txData.fail_reason));
    }

    if (txData.tx_type == 'ChangePubKey') {
        return [
            typeEntry(txData),
            statusEntry(txData),
            fromLinkEntry(txData),
            feeEntry(txData),
            newSignerPubKeyHashEntry(txData),
            createdAtEntry(txData),
            ...rows
        ];
    }

    if (txData.tx_type == 'Deposit' || txData.tx_type == 'FullExit') {
        return [
            typeEntry(txData),
            statusEntry(txData),
            fromLinkEntry(txData),
            toLinkEntry(txData),
            amountEntry(txData),
            ...rows
        ];
    }

    return [
        typeEntry(txData),
        statusEntry(txData),
        fromLinkEntry(txData),
        toLinkEntry(txData),
        amountEntry(txData),
        feeEntry(txData),
        createdAtEntry(txData),
        ...rows
    ];
}
