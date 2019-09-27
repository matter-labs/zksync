// import { formatUnits } from 'ethers/utils';
import { ethers } from 'ethers'

export function readableEther(wei) {
    return ethers.utils.formatUnits(wei, 18).match(/\d+\.\d{1,3}/)[0];
}

export function getDisplayableBalanceDict(dict) {
    let res = Object.assign({}, dict);
    if (res['ETH'] != undefined)
        res['ETH'] = readableEther(dict['ETH']);
    return res;
}

export function getDisplayableBalanceList(list) {
    return list.map(bal => {
        if (bal.tokenName != 'ETH') return bal;
        let res = Object.assign({}, bal);
        res.amount = readableEther(res.amount);
        return res;
    });
}

export function bigNumberMax(a, b) {
    return a.gt(b) ? a : b;
}

export function feesFromAmount(amount) {
    return [
        0,
        bigNumberMax(amount.div(100), 1),
        bigNumberMax(amount.div(20), 1),
    ].map(String);
}
