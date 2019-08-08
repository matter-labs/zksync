import BN = require('bn.js');
import { integerToFloat } from './utils';
import Axios from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import 'ethers';
import { ethers } from 'ethers';

export type Address = string;
export type Token = number;

class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000') {}

    async submitTx(tx) {
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx).then(reps => reps.data);
    }

    async getState(address: Address) {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address).then(reps => reps.data);
    }
}

export class Wallet {
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Wallet) {
        let privateKey = new BN(HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(altjubjubCurve.n);
        this.publicKey = altjubjubCurve.g.mul(this.privateKey).normalize();
        let [x, y] = [this.publicKey.getX(), this.publicKey.getY()];
        let buff = Buffer.from(x.toString('hex') + y.toString('hex'), 'hex');
        let hash = pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex') + hash.getY().toString('hex')).slice(0, 27 * 2);
    }

    async deposit(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        let tx = {
            type: 'Deposit',
            to: this.address,
            token: token,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async transfer(address: Address, token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        // use packed numbers for signture
        let tx = {
            type: 'Transfer',
            from: this.address,
            to: address,
            token: token,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async getNonce(): Promise<number> {
        let state = await this.provider.getState(this.address);
        if (state.error) {
            return 0;
        }
        return (await this.provider.getState(this.address)).nonce;
    }

    static async fromEthWallet(wallet: ethers.Wallet) {
        let defaultFranklinProvider = new FranklinProvider();
        let seed = (await wallet.signMessage('Matter login')).substr(2);
        let frankinWallet = new Wallet(Buffer.from(seed, 'hex'), defaultFranklinProvider, wallet);
        return frankinWallet;
    }

    async getState() {
        return await this.provider.getState(this.address);
    }
}

async function run() {
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1");
    let wallet = await Wallet.fromEthWallet(ethWallet);
    console.log(wallet);
}
//
// run();
