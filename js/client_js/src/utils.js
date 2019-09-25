import { formatUnits } from 'ethers/utils';

export function readableEther(wei) {
    return formatUnits(wei, 18).match(/\d+\.\d{1,3}/)[0];
}

export function getDisplayableBalanceDict(dict) {
    let res = Object.assign({}, dict);
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
