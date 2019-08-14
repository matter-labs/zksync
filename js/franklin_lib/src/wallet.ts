import BN = require('bn.js');
import { integerToFloat } from './utils';
import Axios from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import 'ethers';
import {Contract, ethers} from 'ethers';
import {franklinContractCode} from "../../../contracts/src.ts/deploy";
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";

const IERC20Conract = require("openzeppelin-solidity/build/contracts/IERC20");

export type Address = string;

interface Token {
    id: number,
    address: string,
    symbol?: string,
}



class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000') {}

    async submitTx(tx) {
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx).then(reps => reps.data);
    }

    async getTokens() {
        return await Axios.get(this.providerAddress + '/api/v0.1/tokens').then(reps => reps.data);
    }

    async getState(address: Address): Promise<FranklinState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address).then(reps => reps.data);
    }
}

interface FranklinAccountState {
    address: Address,
    nonce: number,
    balances: BigNumberish[],
}

interface FranklinState {
    id?: number,
    commited: FranklinAccountState,
    verified: FranklinAccountState,
    pending_txs: any[],
}

export class Wallet {
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;
    supportedTokens;


    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer) {
        let privateKey = new BN(HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(altjubjubCurve.n);
        this.publicKey = altjubjubCurve.g.mul(this.privateKey).normalize();
        let [x, y] = [this.publicKey.getX(), this.publicKey.getY()];
        let buff = Buffer.from(x.toString('hex') + y.toString('hex'), 'hex');
        let hash = pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex') + hash.getY().toString('hex')).slice(0, 27 * 2);
    }

    async depositOnchain(token: Token, amount: BigNumber) {
        const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
        const franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
        if (token.id == 0) {
            const tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: amount});
            return await tx.hash;
        } else {
            const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
            await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
            const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, franklinAddressBinary,
                {gasLimit: bigNumberify("150000")});
            return await tx.hash;
        }
    }

    async depositOffchain(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        let tx = {
            type: 'Deposit',
            to: this.address,
            token: token.id,
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
        return (await this.provider.getState(this.address)).commited.nonce
    }

    static async fromEthWallet(wallet: ethers.Signer) {
        let defaultFranklinProvider = new FranklinProvider();
        let seed = (await wallet.signMessage('Matter login')).substr(2);
        let frankinWallet = new Wallet(Buffer.from(seed, 'hex'), defaultFranklinProvider, wallet);
        return frankinWallet;
    }

    async getState() {
        this.supportedTokens = await this.provider.getTokens();
        return await this.provider.getState(this.address);
    }
}

async function run() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let wallet = await Wallet.fromEthWallet(ethWallet);
    console.log((await wallet.getState()));
    return;
    console.log(await wallet.depositOnchain(wallet.supportedTokens['1'], bigNumberify(1)));
    await wallet.depositOffchain(wallet.supportedTokens['1'], new BN(1), new BN(0));
    console.log((await wallet.getState()));
}

run();
