export function isTxPriorityOp(tx) {
    return tx.op.type === 'Deposit' || tx.op.type === 'FullExit';
}

export function getFromAddressOfTx(tx) {
    if (tx.op.type === 'Deposit') {
        return tx.op.priority_op.from;
    }

    if (tx.op.type === 'FullExit') {
        return tx.op.priority_op.eth_address;
    }

    if (tx.op.type === 'ChangePubKey') {
        return tx.op.account;
    }

    if (tx.op.type === 'ForcedExit') {
        return tx.op.target;
    }

    return tx.op.from;
}

const txFromDefault = {
    Deposit: 'unknown sender',
    Transfer: 'unknown from',
    ChangePubKey: 'unknown account address',
    Withdraw: 'unknown account',
    ForcedExit: 'unknown account',
    FullExit: 'unknown account address'
};

export function getTxFromFallbackValue(tx) {
    const fromDefault = txFromDefault[tx.op.type];

    if (!fromDefault) {
        noFallbackError(tx.op.type, 'From');
    }

    return fromDefault;
}

export function getTxToAddress(tx) {
    if (tx.op.type === 'Deposit') {
        return tx.op.priority_op.to;
    }

    if (tx.op.type === 'FullExit') {
        return tx.op.priority_op.eth_address;
    }

    if (tx.op.type === 'ChangePubKey') {
        return tx.op.newPkHash;
    }

    if (tx.op.type === 'ForcedExit') {
        return tx.op.target;
    }

    return tx.op.to;
}

const txToFallback = {
    Deposit: 'unknown account',
    Transfer: 'unknown to',
    ChangePubKey: 'unknown pubkey hash',
    Withdraw: 'unknown ethAddress',
    ForcedExit: 'unknown ethAddress',
    FullExit: 'unknown account address'
};

export function getTxToFallbackValue(tx) {
    const fallback = txToFallback[tx.op.type];

    if (!fallback) {
        noFallbackError(tx.op.type, 'To');
    }
}

export function getTxToken(tx) {
    if (isTxPriorityOp(tx)) {
        return tx.op.priority_op.token;
    }

    if (tx.op.type === 'ChangePubKey') {
        return tx.op.feeToken || 0;
    }

    return tx.op.token;
}

export async function getTxAmount(tx, client) {
    if (tx.op.type === 'Deposit') {
        return tx.op.priority_op.amount;
    }

    if (tx.op.type === 'ChangePubKey') {
        return 'unknown amount';
    }

    // TODO: Remove the hack to get the amount field in ForcedExit operations (ZKS-112).
    if (tx.op.type === 'ForcedExit') {
        return (await client.searchTx(tx.tx_hash)).amount;
    }

    if (tx.op.type === 'FullExit') {
        return tx.op.withdraw_amount;
    }

    return tx.op.amount;
}

export function getTxFee(tx) {
    if (isTxPriorityOp(tx)) {
        return null;
    }

    return tx.op.fee;
}

function noFallbackError(type, field) {
    throw new Error(`No fallback \`${field}\` value for type ${type}`);
}

export function numOrZero(num) {
    // +num converts it to a number
    // It is likely that the number won't be precise
    // but we don't care since we are basically checking
    // if num is a number at all
    if (!isFinite(+num)) {
        return 0;
    }

    // Usually num will be a BigNumber, that's why we simply
    // return `num` instead of +num, since loss of precision is
    // critical here
    return num;
}
