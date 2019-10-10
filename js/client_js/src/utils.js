// import { formatUnits } from 'ethers/utils';
import { ethers } from 'ethers'

export const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export function readableEther(wei) {
    let formatted = ethers.utils.formatUnits(wei, 18);
    if (formatted.startsWith('0.') == false) {
        return formatted.match(/\d+\.\d{1,3}/)[0];
    } else {
        return formatted.match(/0\.0*[^0]{0,3}/)[0];
    }
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
export function bigNumberMin(a, b) {
    return a.gt(b) ? a : b;
}

export function feesFromAmount(amount) {
    return [
        ethers.utils.bigNumberify(0),
        amount.div(100),
        amount.div(20),
    ].map(String);
}

export function strCompareIgnoreCase(a, b) {
    if (a != undefined) a = a.toLowerCase()
    if (b != undefined) b = b.toLowerCase()
    return a == b;
}
