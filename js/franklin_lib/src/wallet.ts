import BN = require('bn.js');
import { integerToFloat } from './utils';
import Axios, { CancelTokenSource } from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import 'ethers';
import {Contract, ethers} from 'ethers';
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";

const franklinContractCode = require("/Users/oleg/Desktop/franklin/contracts/build/Franklin")
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

    async getState(address: Address): Promise<FranklinAccountState> {
        return await Axios.get(this.providerAddress + '/api/v0.1/account/' + address).then(reps => reps.data);
    }
}

interface FranklinAccountState {
    address: Address,
    nonce: number,
    balances: BigNumberish[],
}

interface FranklinAccountState {
    id?: number,
    commited: FranklinAccountState,
    verified: FranklinAccountState,
    pending_txs: any[],
}
interface ETHAccountState {
    onchainBalances: BigNumberish[],
    contractBalances: BigNumberish[],
    lockedBlocksLeft: BigNumberish[],
}

export class Wallet {
    static tokensNames = ['ETH', 'ERC20'];
    static tokensAddresses = ['eth_address', '0x06Fc308DB909c1Fe016243b623CFa42c50487e07'];
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;
    contract: ethers.Contract;

    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;

    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Wallet) {
        let privateKey = new BN(HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(altjubjubCurve.n);
        this.publicKey = altjubjubCurve.g.mul(this.privateKey).normalize();
        let [x, y] = [this.publicKey.getX(), this.publicKey.getY()];
        let buff = Buffer.from(x.toString('hex') + y.toString('hex'), 'hex');
        let hash = pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex') + hash.getY().toString('hex')).slice(0, 27 * 2);
        this.contract = new ethers.Contract(
            "0xDE1F1506b9b881DE029D4BD79745DDD4E16caa97", 
            require('/Users/oleg/Desktop/franklin/contracts/build/Franklin').abi, 
            ethWallet);
        this.contract.connect(this.ethWallet);
    }

    async depositOnchain(token: Token, amount: BigNumber) {
        const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
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

    /**
     * transfer from contract balance to franklin balance
     * @param token 
     * @param amount 
     * @param fee 
     */
    async depositOffchain(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        if (this.franklinState.pending_txs.length > 0) {
            console.log("please wait for all pending transactions to complete before sending a new one.");
            return;
        }

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

    /**
     * transfer from contract to onchain
     * @param token 
     * @param amount 
     */
    async widthdrawOnchain(token: Token, amount: BigNumber) {
        const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
        const franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
        if (token.id == 0) {
            const tx = await franklinDeployedContract.withdrawETH(amount);
            await tx.wait(2);
            return tx.hash;
        } else {
            const tx = await franklinDeployedContract.withdrawERC20(token.address, amount, {gasLimit: bigNumberify("150000")});
            await tx.wait(2);
            return tx.hash;
        }
    }

    /**
     * from b
     * @param token 
     * @param amount 
     * @param fee 
     */
    async widthdrawOffchain(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        if (this.franklinState.pending_txs.length > 0) {
            console.log("please wait for all pending transactions to complete before sending a new one.");
            return;
        }

        let tx = {
            type: 'Withdraw',
            account: this.address,
            eth_address: await this.ethWallet.getAddress(),
            token: token.id,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async transfer(address: Address, token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        if (this.franklinState.pending_txs.length > 0) {
            console.log("please wait for all pending transactions to complete before sending a new one.");
            return;
        }
        // use packed numbers for signture
        let tx = {
            type: 'Transfer',
            from: this.address,
            to: address,
            token: token.id,
            amount: amount.toString(10),
            fee: fee.toString(10),
            nonce: nonce,
        };

        return await this.provider.submitTx(tx);
    }

    async getNonce(): Promise<number> {
        await this.getState();
        return this.franklinState.commited.nonce
    }

    static async fromEthWallet(wallet: ethers.Wallet) {
        let defaultFranklinProvider = new FranklinProvider();
        let seed = await wallet.signMessage('Matter login');
        console.log('seed', seed);
        let gex = Buffer.from(seed.substr(2), 'hex');
        let frankinWallet = new Wallet(gex, defaultFranklinProvider, wallet);
        return frankinWallet;
    }

    async getState() {
        this.supportedTokens = await this.provider.getTokens();
        this.franklinState = await this.provider.getState(this.address);
    }

    private state_ = null;
    private state_timestamp_ = null;
    private async state() {
        const update_interval = 1000;
        let curr_time = Date.now();
        if (this.state_timestamp_ === null || curr_time - this.state_timestamp_ > update_interval) {
            this.state_ = await this.getState();
            this.state_timestamp_ = curr_time;
        }
        return this.state_;
    }

    /**
     * gets balance for token in the mainchain
     * @param token — tokenId
     */
    async getOnchainBalanceForToken(token: Token) {
        if (token.id === 0) {
            return await this.ethWallet.getBalance();
        }

        let erc20abi = require('./erc20.abi');
        let contract = new ethers.Contract(token.address, erc20abi, this.ethWallet);
        return await contract.balanceOf(this.ethWallet.address);
    }

    /**
     * returns a list of tokenIds that user has in his mainchain account
     */
    async getOnchainTokensList() {
        // user should add tokens by hand to view their balance
        // just like in metamask. We have to store it somewhere, idk.
        // for now, hardcode.
        await this.getState();
        return this.supportedTokens;
        // return [0, 1];
    }

    /**
     * get a list of balances in the mainchain
     */
    async getCommittedOnchainState() {
        let tokens = await this.getOnchainTokensList();
        // let balanceGetter = this.getOnchainBalanceForToken.bind(this);
        // let balances = await Promise.all(tokens.map(balanceGetter));
        let res = [];
        for (let t = 0; t < tokens.length; ++t) {
            let currToken = this.supportedTokens[t];
            let balance = (await this.getOnchainBalanceForToken(currToken)).toString(10);
            res.push({
                token: currToken.address,
                balance: balance,
            });
        }
        return {
            balances: res
        };
    }

    async getPendingOnchainState() {
        return await this.getCommittedOnchainState();
    }

    /**
     * list of tokens user has locked/unlocked in our contract
     */
    async getCommittedContractTokensList() {
        // TODO:
        // this can be retrieved from our contract or our server
        return await this.getOnchainTokensList();
    }

    async getLockedContractBalanceForToken(token: Token) {
        let [balance, block] = await this.contract.getMyBalanceForToken(token);
        let currBlock = await this.ethWallet.provider.getBlockNumber();
        if (currBlock < block) {
            return balance;
        }
        return new BN(0);
    }

    async getUnlockedContractBalanceForToken(token: Token) {
        let address = this.ethWallet.address;
        let [balance, block] = await this.contract.balances(address, token.id);
        let currBlock = await this.ethWallet.provider.getBlockNumber();
        if (currBlock >= block) {
            return balance;
        }
        return new BN(0);
    }

    /**
     * locked/unlocked balances in our contract
     */
    async getCommittedContractBalances() {
        let tokens: Token[] = await this.getCommittedContractTokensList();
        let lockedBalanceGetter = this.getLockedContractBalanceForToken.bind(this);
        let unlockedBalanceGetter = this.getUnlockedContractBalanceForToken.bind(this);

        let res = [];
        for (let i = 0; i < tokens.length; i++) {
            let token: Token = tokens[i];
            let lockedBalance = await this.getLockedContractBalanceForToken(token);
            let unlockedBalance = await this.getUnlockedContractBalanceForToken(token);
            res.push({
                token: token.symbol,
                locked: lockedBalance,
                unlocked: unlockedBalance
            });
        }

        // let [lockedBalances, unlockedBalances] = await Promise.all([
        //     Promise.all(tokens.map(lockedBalanceGetter)),
        //     Promise.all(tokens.map(unlockedBalanceGetter))
        // ]);
        // let res = [];
        // for (let t = 0; t < tokens.length; ++t) {
        //     res.push({
        //         token: Wallet.tokensNames[t],
        //         locked: lockedBalances[t].toString(),
        //         unlocked: unlockedBalances[t].toString()
        //     });
        // }
        // return {
        //     contractBalances: res
        // };
    }

    /**
     * locked/unlocked balances, but pending.
     * ought to be calculated from the transactions sent from the wallet 
     * during this very session.
     */
    async getPendingContractBalances() {
        return await this.getCommittedContractBalances();
    }

    /**
     * balances in Franklin
     */
    async getVerifiedFranklinState() {
        let res = await this.getCommittedOnchainState();
        res['nonce'] = 32;
        return res;
    }

    /**
     * 
     */
    async getCommittedFranklinState() {
        return await this.getVerifiedFranklinState();
    }
    
    /**
     * take verified state and here, on the client, apply transactions 
     * sent during this session, to get the pending state.
     */
    async getPendingFranklinState() {
        return await this.getVerifiedFranklinState();
    }

    /**
     * transfer from contract balance to franklin
     * @param token 
     * @param amount 
     */
    async depositFranklin(token, amount) {
        
    }

    /**
     * transfer from Franklin balance to the contract balance
     * @param token 
     * @param amount 
     */
    async exitFranklin(token, amount) {

    }

    /**
     * transfer from our contract balance to the mainchain balance
     * @param token 
     * @param amount 
     */
    async exitOnchain(token, amount) {

    }

    /**
     * load all the transactions for user
     */
    async getFullTransactionHistory() {
        return this.getLocalTransactionHistory()
    }

    /**
     * just the transactions sent during this session
     */
    getLocalTransactionHistory() {
        return [
            {
                txId:   0xDEADBEEF,
                to:     0x00000000,
                token:  0,
                amount: "20",
                fee:    "3", 
                status: "pending" // 'pending' | 'failed' | 'committed' | 'verified'
            }
        ]
    }
}
