import Axios from 'axios';
import {
    Network,
    TxEthSignature
} from './types';

export function getDefaultRestProvider(network: Network): RestProvider {
    if (network === 'localhost') {
        return new RestProvider('http://127.0.0.1:3001/api/v0.2');
    } else if (network === 'ropsten') {
        return new RestProvider('https://ropsten-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby') {
        return new RestProvider('https://rinkeby-api.zksync.io/api/v0.2');
    } else if (network === 'ropsten-beta') {
        return new RestProvider('https://ropsten-beta-api.zksync.io/api/v0.2');
    } else if (network === 'rinkeby-beta') {
        return new RestProvider('https://rinkeby-beta-api.zksync.io/api/v0.2');
    } else if (network === 'mainnet') {
        return new RestProvider('https://api.zksync.io/api/v0.2');
    } else {
        throw new Error(`Ethereum network ${network} is not supported`);
    }
}


export class RestProvider {
    public constructor(public address: string) {}
    // return transaction hash (e.g. sync-tx:dead..beef)
    async submitTx(tx: any, signature?: TxEthSignature): Promise<string> {
        const response = await Axios.post(this.address + '/transaction', {tx, signature}).then((resp) => {
            return resp.data;
        });
    }
}