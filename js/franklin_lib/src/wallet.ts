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
    pendingTxs: any[],
}
interface ETHAccountState {
    onchainBalances: BigNumberish[],
    contractBalances: BigNumberish[],
    lockedBlocksLeft: BigNumberish[],
}

export class Wallet {
    static tokensNames = ['ETH', 'ERC20'];
    static tokensAddresses = ['eth_address', '0x572b9410D9a14Fa729F3af92cB83A07aaA472dE0'];
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;
    ethWallet: ethers.Wallet;
    contract: ethers.Contract;

    supportedTokens: Token[];
    franklinState: FranklinAccountState;
    ethState: ETHAccountState;



    constructor(seed: Buffer, public provider: FranklinProvider, public ethWallet: ethers.Signer) {
        let privateKey = new BN(HmacSHA512(seed.toString('hex'), 'Matter seed').toString(), 'hex');
        this.privateKey = privateKey.mod(altjubjubCurve.n);
        this.publicKey = altjubjubCurve.g.mul(this.privateKey).normalize();
        this.ethWallet = ethersWallet;
        let [x, y] = [this.publicKey.getX(), this.publicKey.getY()];
        let buff = Buffer.from(x.toString('hex') + y.toString('hex'), 'hex');
        let hash = pedersenHash(buff);
        this.address = '0x' + (hash.getX().toString('hex') + hash.getY().toString('hex')).slice(0, 27 * 2);
        this.contract = new ethers.Contract(
            "5E6D086F5eC079ADFF4FB3774CDf3e8D6a34F7E9", 
            require('../../../contracts/build/Franklin').abi, 
            ethersWallet.provider);
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
        let seed = await wallet.signMessage('Matter login');
        console.log('seed', seed);
        let gex = Buffer.from(seed.substr(2), 'hex');
        let frankinWallet = new Wallet(gex, defaultFranklinProvider, wallet);
        return frankinWallet;
    }

    async getState() {
        return await this.provider.getState(this.address);
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
        if (token === 0) {
            return await this.ethWallet.getBalance();
        }

        let address = Wallet.tokensAddresses[token];
        let erc20abi = require('./erc20.abi');
        let contract = new ethers.Contract(address, erc20abi, this.ethWallet);
        return await contract.balanceOf(this.ethWallet.address);
    }

    /**
     * returns a list of tokenIds that user has in his mainchain account
     */
    async getOnchainTokensList() {
        // user should add tokens by hand to view their balance
        // just like in metamask. We have to store it somewhere, idk.
        // for now, hardcode.
        return [0, 1];
    }

    /**
     * get a list of balances in the mainchain
     */
    async getCommittedOnchainState() {
        let tokens = await this.getOnchainTokensList();
        let balanceGetter = this.getOnchainBalanceForToken.bind(this);
        let balances = await Promise.all(tokens.map(balanceGetter));
        let res = [];
        for (let t = 0; t < tokens.length; ++t) {
            res.push({
                token: Wallet.tokensNames[t],
                balance: balances[t].toString()
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
        return [0];
    }

    async getLockedContractBalanceForToken(token: Token) {
        return new BN(Math.random() * 1000);
    }
    
    async getUnlockedContractBalanceForToken(token: Token) {
        return new BN(Math.random() * 1000);
    }

    /**
     * locked/unlocked balances in our contract
     */
    async getCommittedContractBalances() {
        let tokens = await this.getCommittedContractTokensList();
        let lockedBalanceGetter = this.getLockedContractBalanceForToken.bind(this);
        let unlockedBalanceGetter = this.getUnlockedContractBalanceForToken.bind(this);
        let [lockedBalances, unlockedBalances] = await Promise.all([
            Promise.all(tokens.map(lockedBalanceGetter)),
            Promise.all(tokens.map(unlockedBalanceGetter))
        ]);
        let res = [];
        for (let t = 0; t < tokens.length; ++t) {
            res.push({
                token: Wallet.tokensNames[t],
                locked: lockedBalances[t].toString(),
                unlocked: unlockedBalances[t].toString()
            });
        }
        return {
            contractBalances: res
        };
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
     * transfer from mainchain to contract
     */
    async depositOnchain(token, amount) {

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

async function run() {
    const provider = new ethers.providers.JsonRpcProvider(process.env.WEB3_URL);
    let ethWallet = ethers.Wallet.fromMnemonic(process.env.MNEMONIC, "m/44'/60'/0'/0/1").connect(provider);
    let wallet = await Wallet.fromEthWallet(ethWallet);
    console.log((await wallet.getState()));

    console.log(await wallet.depositOnchain(wallet.supportedTokens['1'], bigNumberify(1)));
    console.log(await wallet.depositOffchain(wallet.supportedTokens['1'], new BN(1), new BN(0)));

    console.log((await wallet.getState()));
}

run();
