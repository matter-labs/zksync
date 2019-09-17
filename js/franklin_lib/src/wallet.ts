import BN = require('bn.js');
import Axios from 'axios';
import {
    musigPedersen,
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress,
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
const IERC20Conract = require("../abi/IERC20");
const franklinContractCode = require("../abi/Franklin");

export type Address = Buffer;


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
        return await Axios.post(this.providerAddress + '/api/v0.1/submit_tx', tx).then(reps => reps.data);
    }

    async getTokens() {
        return await Axios.get(this.providerAddress + '/api/v0.1/tokens').then(reps => reps.data);
    }

    async getState(address: Address): Promise<FranklinAccountState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address).then(reps => reps.data);
    }
}

export interface Token {
    id: number,
    address: string,
    symbol?: string,
}

export interface FranklinAccountState {
    address: Address,
    nonce: number,
    balances: BigNumber[],
}

export interface FranklinAccountState {
    id?: number,
    commited: FranklinAccountState,
    verified: FranklinAccountState,
    pending_txs: any[],
}
interface ETHAccountState {
    onchainBalances: BigNumber[],
    contractBalances: BigNumber[],
    lockedBlocksLeft: number[],
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
        return musigPedersen(this.privateKey, msg);
    }

    signWithdraw(tx: WithdrawTx) {
        let type = Buffer.from([3]);
        let account = tx.account;
        let eth_address = Buffer.from(tx.eth_address.slice(2),"hex")
        let token = Buffer.alloc(2);
        token.writeUInt16BE(tx.token,0);
        let bnAmount = new BN(bigNumberify(tx.amount).toString());
        let amount = bnAmount.toBuffer("be", 16);
        let bnFee = new BN(bigNumberify(tx.fee).toString());
        let fee = packFee(bnFee);

        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return musigPedersen(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        let type = Buffer.from([4]);
        let account = tx.account;
        let nonce = Buffer.alloc(4);
        nonce.writeUInt32BE(tx.nonce, 0);

        let msg = Buffer.concat([type, account, nonce]);
        return musigPedersen(this.privateKey, msg);
    }
}


export class Wallet {
    address: Address;
    walletKeys: WalletKeys;

    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer, public ethAddress: string) {
        let {privateKey} = privateKeyFromSeed(seed);
        this.walletKeys = new WalletKeys(privateKey);
        this.address = pubkeyToAddress(this.walletKeys.publicKey);
    }

    async deposit(token: Token, amount: BigNumberish) {
        // const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        // const franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
        // if (token.id == 0) {
        //     const tx = await franklinDeployedContract.depositETH(franklinAddressBinary, {value: amount});
        //     return tx.hash;
        // } else {
        //     const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
        //     await erc20DeployedToken.approve(franklinDeployedContract.address, amount);
        //     const tx = await franklinDeployedContract.depositERC20(erc20DeployedToken.address, amount, franklinAddressBinary,
        //         {gasLimit: bigNumberify("150000"), value: parseEther("0.001")});
        //     return tx.hash;
        // }
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

        return await this.provider.submitTx(tx);
    }

    async emergencyWithdraw(token: Token) {
        const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        // TODO: use signature, estimate fee?
        await this.fetchFranklinState();
        let tx = await franklinDeployedContract.fullExit(this.franklinState.id, token.address,  Buffer.alloc(64, 7),
            {gasLimit: bigNumberify("200000"), value: parseEther("0.02")});
        return tx.hash;
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
        await this.fetchFranklinState();
        return this.franklinState.commited.nonce
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

        // const franklinDeployedContract = new Contract(this.provider.contractAddress, franklinContractCode.interface, this.ethWallet);
        // for(let token  of this.supportedTokens) {
        //     if (token.id == 0) {
        //         onchainBalances[token.id] = await this.ethWallet.provider.getBalance(this.ethAddress);
        //     } else {
        //         const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
        //         onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethAddress).then(n => n.toString());
        //     }
        //     const balanceStorage = await franklinDeployedContract.balances(this.ethAddress, token.id);
        //     contractBalances[token.id] = balanceStorage.balance;
        //     lockedBlocksLeft[token.id] = Math.max(balanceStorage.lockedUntilBlock - currentBlock, 0);
        // }

        this.ethState = {onchainBalances, contractBalances, lockedBlocksLeft};
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
