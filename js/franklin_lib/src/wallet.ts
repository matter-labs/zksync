import BN = require('bn.js');
import { integerToFloat } from './utils';
import Axios from 'axios';

export type Address = Buffer;
export type Token = number;

export class Wallet {
    // TODO: use public/private key instead of address.
    constructor(public address: Address, public franklin_provider: string = 'http://127.0.0.1:3000') {}

    async deposit(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        // use packed numbers for signture
        let tx = {
            type: 'Deposit',
            to: this.address.toString('hex'),
            token: token,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        let result = await Axios.post(this.franklin_provider + '/api/v0.1/submit_tx', tx);
        return result.data;
    }

    async transfer(address: Address, token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        // use packed numbers for signture
        let tx = {
            type: 'Transfer',
            from: this.address.toString('hex'),
            to: address.toString('hex'),
            token: token,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        let result = await Axios.post(this.franklin_provider + '/api/v0.1/submit_tx', tx);
        return result.data;
    }

    async getState() {
        return await Axios.get(this.franklin_provider + '/api/v0.1/account/' + this.address.toString('hex')).then(
            reps => reps.data,
        );
    }

    async getNonce() {
        let state = await this.getState();
        if (state.error) {
            return 0;
        }
        return (await this.getState()).nonce;
    }
}
