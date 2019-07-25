import BN = require('bn.js');
import { integerToFloat } from './utils';
import Axios from 'axios';

type Token = number;

class Wallet {
    nonce: number;

    constructor(public address, public tx_endpoint: string = 'http://127.0.0.1:3000') {
        this.nonce = 0;
    }

    deposit(token: Token, amount: BN, fee: BN) {
        // use packed numbers for signture
        let packed_amount = Buffer.concat([Buffer.from([0]), integerToFloat(amount, 9, 15, 10)]).readUInt32BE(0);
        let packed_fee = integerToFloat(fee, 4, 4, 10).readUInt8(0);
        let tx = {
            type: 'Deposit',
            to: this.address,
            token: token,
            amount: packed_amount,
            fee: packed_fee,
            nonce: this.nonce,
        };
        this.nonce += 1;

        return Axios.post(this.tx_endpoint + '/api/v0.1/submit_tx', tx);
    }
}

async function main() {
    let wallet = new Wallet({
        data: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    });

    let result = await wallet.deposit(0, new BN(1200), new BN(8));
    console.log(result);
}

main();
