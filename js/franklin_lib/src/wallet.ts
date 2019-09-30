import BN = require('bn.js');
import Axios from 'axios';
import {
    musigPedersen,
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress, serializePointPacked, signTransactionBytes,
} from './crypto';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import {Contract, ethers} from 'ethers';
import edwards = curve.edwards;
import {packAmount, packFee} from "./utils";

// ! can't import from 'ethers/utils' it won't work in the browser.
type BigNumber = ethers.utils.BigNumber;
type BigNumberish = ethers.utils.BigNumberish;
const parseEther = ethers.utils.parseEther;
const bigNumberify = ethers.utils.bigNumberify;
const PUBKEY_HASH_LEN=20;
const IERC20Conract = require("../abi/IERC20.json");
const franklinContractCode = require("../abi/Franklin.json");

export type Address = Buffer;
export type AddressLike = Buffer | string;

export function toAddress(addressLike: AddressLike): Address {
    if (typeof(addressLike) == "string") {
        return Buffer.from(addressLike.substr(2),"hex");
    } else {
        return addressLike;
    }
}

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000', public contractAddress: string = process.env.CONTRACT_ADDR) {}

    static prepareTransferRequestForNode(tx: TransferTx, signature) {
        let req: any = tx;
        req.type = "Transfer";
        req.from = `0x${tx.from.toString("hex")}`;
        req.to = `0x${tx.to.toString("hex")}`;
        req.amount = bigNumberify(tx.amount).toString();
        req.fee = bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareWithdrawRequestForNode(tx: WithdrawTx, signature) {
        let req: any = tx;
        req.type = "Withdraw";
        req.account = `0x${tx.account.toString("hex")}`;
        req.amount = bigNumberify(tx.amount).toString();
        req.fee = bigNumberify(tx.fee).toString();
        req.signature = signature;
        return req;
    }

    static prepareCloseRequestForNode(tx: CloseTx, signature) {
        let req: any = tx;
        req.type = "Close";
        req.account = `0x${tx.account.toString("hex")}`;
        req.signature = signature;
        return req;
    }

    async submitTx(tx) {
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx)
            .then(reps => reps.data)
            .catch(error => console.log(error.response));
    }

    async getTokens() {
        return await Axios.get(this.providerAddress + '/api/v0.1/tokens')
            .then(reps => reps.data)
            .catch(error => console.log(error.response));
    }

    async getTransactionsHistory(address: Address) {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + `0x${address.toString("hex")}` + '/transactions')
            .then(reps => reps.data)
            .catch(error => console.log(error.response));
    }

    async getState(address: Address): Promise<FranklinAccountState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + `0x${address.toString("hex")}`)
            .then(reps => reps.data)
            .catch(error => console.log(error.response));
    }

    async getTxReceipt(tx_hash) {
        return await Axios.get(this.providerAddress + '/api/v0.1/transactions/' + tx_hash)
            .then(reps => reps.data)
            .catch(error => console.log(error.response));
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
}
interface ETHAccountState {
    onchainBalances: BigNumber[],
    contractBalances: BigNumber[],
}

export interface TransferTx {
    from: Address,
    to: Address,
    token: number,
    amount: BigNumberish,
    fee: BigNumberish,
    nonce: number,
}

export interface WithdrawTx {
    account: Address,
    eth_address: String,
    token: number,
    amount: BigNumberish,
    fee: BigNumberish,
    nonce: number,
}

export interface CloseTx {
    account: Address,
    nonce: number,
}

export interface FullExitReq {
    token: number,
    eth_address: String
    nonce: number,
}

export class WalletKeys {
    publicKey: edwards.EdwardsPoint;

    constructor(public privateKey: BN) {
        this.publicKey = privateKeyToPublicKey(privateKey);
    }

    signTransfer(tx: TransferTx) {
        let type = Buffer.from([5]); // tx type
        let from = tx.from;
        let to = tx.to;
        let token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token,0);
        let bnAmount = new BN(bigNumberify(tx.amount).toString() );
        let amount = packAmount(bnAmount);
        let bnFee = new BN(bigNumberify(tx.fee).toString());
        let fee = packFee(bnFee);
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);
        let msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signWithdraw(tx: WithdrawTx) {
        let type = Buffer.from([3]);
        let account = tx.account;
        let eth_address = Buffer.from(tx.eth_address.slice(2),"hex")
        let token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token,0);
        let bnAmount = new BN(bigNumberify(tx.amount).toString());
        let amount = bnAmount.toArrayLike(Buffer, "be", 16);
        let bnFee = new BN(bigNumberify(tx.fee).toString());
        let fee = packFee(bnFee);

        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        let type = Buffer.from([4]);
        let account = tx.account;
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signFullExit(op: FullExitReq) {
        let type = Buffer.from([6]);
        let packed_pubkey = serializePointPacked(this.publicKey);
        let eth_address = Buffer.from(op.eth_address.slice(2),"hex")
        let token = Buffer.alloc(2);
        token.writeUInt16BE(op.token,0);
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(op.nonce, 0);
        let msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(signTransactionBytes(this.privateKey, msg).sign, "hex");
    }
}


export class Wallet {
    address: Address;
    walletKeys: WalletKeys;

    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;
    pendingNonce: number;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer, public ethAddress: string) {
        let {privateKey} = privateKeyFromSeed(seed);
        this.walletKeys = new WalletKeys(privateKey);
        this.address = pubkeyToAddress(this.walletKeys.publicKey);
    }

    async deposit(token: Token, amount: BigNumberish) {
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        if (token.id == 0) {
            const tx = await franklinDeployedContract.depositETH(this.address, {value: amount, gasLimit: bigNumberify("200000")});
            return tx.hash;
        } else {
            const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
            await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
            const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, this.address,
                {gasLimit: bigNumberify("300000"), value: parseEther("0.05")});
            return tx.hash;
        }
    }

    async waitTxReceipt(tx_hash) {
        while (true) {
            let receipt = await this.provider.getTxReceipt(tx_hash);
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
        let tx = {
            account: this.address,
            eth_address: await this.ethWallet.getAddress(),
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(),
        };
        let signature = this.walletKeys.signWithdraw(tx);
        let tx_req = FranklinProvider.prepareWithdrawRequestForNode(tx, signature);

        return await this.provider.submitTx(tx_req);
    }

    async emergencyWithdraw(token: Token) {
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        let nonce = await this.getNonce();
        let signature = this.walletKeys.signFullExit({token: token.id, eth_address: await this.ethWallet.getAddress(), nonce});
        let tx = await franklinDeployedContract.fullExit(serializePointPacked(this.walletKeys.publicKey), token.address,  signature, nonce,
            {gasLimit: bigNumberify("500000"), value: parseEther("0.02")});
        return tx.hash;
    }

    async transfer(to: AddressLike, token: Token, amount: BigNumberish, fee: BigNumberish) {
        let tx = {
            from: this.address,
            to: toAddress(to),
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(),
        };
        let signature = this.walletKeys.signTransfer(tx);
        let tx_req = FranklinProvider.prepareTransferRequestForNode(tx, signature);

        return await this.provider.submitTx(tx_req);
    }

    async close() {
        let tx = {
            account: this.address,
            nonce: await this.getNonce()
        };

        let signature = this.walletKeys.signClose(tx);
        let tx_req = FranklinProvider.prepareCloseRequestForNode(tx, signature);

        return await this.provider.submitTx(tx_req);
    }

    async getNonce(): Promise<number> {
        // TODO: reconsider nonce logic 
        if (this.pendingNonce == null) {
            await this.fetchFranklinState();
            this.pendingNonce = this.franklinState.commited.nonce + this.franklinState.pending_txs.length;
        }
        return this.pendingNonce++;
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

        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        for (let token of this.supportedTokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.provider.getBalance(this.ethAddress);
            } else {
                const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
                onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethAddress).then(n => n.toString());
            }
            contractBalances[token.id] = await franklinDeployedContract.balancesToWithdraw(this.ethAddress, token.id);
        }

        this.ethState = {onchainBalances, contractBalances};
    }

    async fetchFranklinState() {
        this.supportedTokens = await this.provider.getTokens();
        this.franklinState = await this.provider.getState(this.address);
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
