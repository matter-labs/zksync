import BN = require('bn.js');
import Axios from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import {Contract, ethers} from 'ethers';

// ! can't import from 'ethers/utils' it won't work in the browser.
type BigNumber = ethers.utils.BigNumber;
type BigNumberish = ethers.utils.BigNumberish;
const bigNumberify = ethers.utils.bigNumberify;
const PUBKEY_HASH_LEN=20;
const IERC20Conract = require("openzeppelin-solidity/build/contracts/ERC20Mintable.json");
const franklinContractCode = require("../abi/Franklin.json");

export type Address = string;

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000', public contractAddress: string = process.env.CONTRACT_ADDR) {}

    async submitTx(tx) {
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx).then(reps => reps.data);
    }

    async getTokens() {
        return await Axios.get(this.providerAddress + '/api/v0.1/tokens').then(reps => reps.data);
    }

    async getTransactionsHistory(address: Address) {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address + '/transactions').then(reps => reps.data);
    }

    async getState(address: Address): Promise<FranklinAccountState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address).then(reps => reps.data);
    }

    async txReceipt(tx_hash) {
        return await Axios.get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash).then(reps => reps.data);
    }
}

export interface Token {
    id: number,
    address: string,
    symbol?: string,
}

export interface FranklinAccountBalanceState {
    address: Address,
    nonce: number,
    balances: BigNumber[],
}

export interface FranklinAccountState {
    id?: number,
    commited: FranklinAccountBalanceState,
    verified: FranklinAccountBalanceState,
    pending_txs: any[],
    tx_history: any[],
}
interface ETHAccountState {
    onchainBalances: BigNumber[],
    contractBalances: BigNumber[],
    lockedBlocksLeft: number[],
}

export class Wallet {
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;

    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;
    nonce: number;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer, public ethAddress: string) {
        let privateKey = new BN(HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(altjubjubCurve.n);
        this.publicKey = altjubjubCurve.g.mul(this.privateKey).normalize();
        let [x, y] = [this.publicKey.getX(), this.publicKey.getY()];
        let buff = Buffer.from(x.toString('hex').padStart(64,'0') + y.toString('hex').padStart(64, '0'), 'hex');
        let hash = pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex').padStart(64, '0') + hash.getY().toString('hex').padStart(64,'0')).slice(0, PUBKEY_HASH_LEN * 2);
        

        this.nonce = null;
    }

    async depositOnchain(token: Token, amount: BigNumberish) {
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        const franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
        if (token.id == 0) {
            const tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: amount});
            await tx.wait(2);
            return tx.hash;
        } else {
            const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
            await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
            const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, franklinAddressBinary,
                {gasLimit: bigNumberify("150000")});
            await tx.wait(2);
            return tx.hash;
        }
    }

    async depositOffchain(token: Token, amount: BigNumberish, fee: BigNumberish) {
        let nonce = await this.getNonce();
        let tx = {
            type: 'Deposit',
            to: this.address,
            token: token.id,
            amount: bigNumberify(amount).toString(),
            fee: bigNumberify(fee).toString(),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async txReceipt(tx_hash) {
        while (true) {
            let receipt = await this.provider.txReceipt(tx_hash);
            if (receipt != null) {
                return receipt
            }
            await sleep(1000);
        }
    }


    async widthdrawOnchain(token: Token, amount: BigNumberish) {
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        if (token.id == 0) {
            const tx = await franklinDeployedContract.withdrawETH(amount, {gasLimit: 200000});
            await tx.wait(2);
            return tx.hash;
        } else {
            const tx = await franklinDeployedContract.withdrawERC20(token.address, amount, {gasLimit: bigNumberify("150000")});
            await tx.wait(2);
            return tx.hash;
        }
    }

    async widthdrawOffchain(token: Token, amount: BigNumberish, fee: BigNumberish) {
        let nonce = await this.getNonce();
        let tx = {
            type: 'Withdraw',
            account: this.address,
            eth_address: await this.ethWallet.getAddress(),
            token: token.id,
            amount: bigNumberify(amount).toString(),
            fee: bigNumberify(fee).toString(),
            nonce: nonce,
        };

        let res = await this.provider.submitTx(tx);
        return res;
    }

    async transfer(address: Address, token: Token, amount: BigNumberish, fee: BigNumberish) {
        let nonce = await this.getNonce();
        // use packed numbers for signature
        let tx = {
            type: 'Transfer',
            from: this.address,
            to: address,
            token: token.id,
            amount: bigNumberify(amount).toString(),
            fee: bigNumberify(fee).toString(),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async getNonce(): Promise<number> {
        if (this.nonce === null) {
            await this.fetchFranklinState();
            this.nonce = this.franklinState.commited.nonce + this.franklinState.pending_txs.length;
        }
        return this.nonce++;
    }

    static async fromEthWallet(wallet: ethers.Signer, franklinProvider: FranklinProvider = new FranklinProvider()) {
        let seed = (await wallet.signMessage('Matter login')).substr(2);
        let ethAddress = await wallet.getAddress();
        let frankinWallet = new Wallet(Buffer.from(seed, 'hex'), franklinProvider, wallet, ethAddress);
        return frankinWallet;
    }

    async fetchEthState() {
        let onchainBalances = new Array<BigNumber>(this.supportedTokens.length);
        let contractBalances = new Array<BigNumber>(this.supportedTokens.length);
        let lockedBlocksLeft = new Array<number>(this.supportedTokens.length);

        const currentBlock = await this.ethWallet.provider.getBlockNumber();

        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        for(let token  of this.supportedTokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.provider.getBalance(this.ethAddress);
            } else {
                const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
                onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethAddress).then(n => n.toString());
            }
            const balanceStorage = await franklinDeployedContract.balances(this.ethAddress, token.id);
            contractBalances[token.id] = balanceStorage.balance;
            lockedBlocksLeft[token.id] = Math.max(balanceStorage.lockedUntilBlock - currentBlock, 0);
        }

        this.ethState = {onchainBalances, contractBalances, lockedBlocksLeft};
    }

    async fetchFranklinState() {
        this.supportedTokens = await this.provider.getTokens();
        this.franklinState = await this.provider.getState(this.address);
        this.franklinState.tx_history = await this.provider.getTransactionsHistory(this.address);
    }

    async updateState() {
        await this.fetchFranklinState();
        await this.fetchEthState();
    }

    async waitPendingTxsExecuted() {
        await this.fetchFranklinState();
        while (this.franklinState.pending_txs.length > 0) {
            await this.fetchFranklinState();
        }
    }
}
