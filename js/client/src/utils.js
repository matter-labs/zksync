import { ethers } from 'ethers'

export const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

const readablyPrintableTokens = ['ETH', 'FAU'];

export function isReadablyPrintable(tokenName) {
    return readablyPrintableTokens.includes(tokenName);
}

/**
 * If amount >= 1.0, we leave up to 3 digits after comma.
 * If it's less, we leave up to 3 the the most significant 
 * digits of the fraction part of the amount.
 * 
 * examples:
 * '0.0000128748239817239486128' => '0.0000128'
 * '1.00232132738' => '1.002'
 */
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
    for (let token of readablyPrintableTokens) {
        if (res[token] != undefined) {
            res[token] = readableEther(dict[token]);
        }
    }
    return res;
}

export function getDisplayableBalanceList(list) {
    return list.map(bal => {
        if (isReadablyPrintable(bal.tokenName) == false) 
            return bal;
        let res = Object.assign({}, bal);
        res.amount = readableEther(res.amount);
        return res;
    });
}

export function bigNumberMax(a, b) {
    return a.gt(b) ? a : b;
}
export function bigNumberMin(a, b) {
    return a.lt(b) ? a : b;
}

export function feesFromAmount(amount) {
    return [
        ethers.utils.bigNumberify(0),
        amount.div(100),
        amount.div(20),
    ].map(String);
}
