import BN = require('bn.js');
import Axios from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import 'ethers';
import {Contract, ethers} from 'ethers';
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";

const franklinContractCode = require('../../../contracts/build/Franklin')
const IERC20Conract = require("openzeppelin-solidity/build/contracts/IERC20");
// import {franklinContractCode} from "../../../contracts/src.ts/deploy";

export type Address = string;

interface Token {
    id: number,
    address: string,
    symbol?: string,
}



class FranklinProvider {
    constructor(public providerAddress: string = 'http://127.0.0.1:3000') {}

    async submitTx(tx) {
        console.log('submitting tx', tx);
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
            process.env.CONTRACT_ADDR,
            require('../../../contracts/build/Franklin').abi, 
            ethWallet);
        this.contract.connect(this.ethWallet);
    }

    async depositOnchain(token: Token, amount: BigNumber) {
        const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
        const franklinAddressBinary = Buffer.from(this.address.substr(2), "hex");
        if (token.id == 0) {
            // console.log(await franklinDeployedContract.balances(this.ethWallet.address, 0));
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
            return {
                err: "please wait for all pending transactions to complete before sending a new one."
            };
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

    /**
     * from b
     * @param token 
     * @param amount 
     * @param fee 
     */
    async widthdrawOffchain(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        if (this.franklinState.pending_txs.length > 0) {
            return {
                error: "please wait for all pending transactions to complete before sending a new one."
            };
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

        console.log('submitting withdrawOffchain tx' + JSON.stringify(tx));

        return await this.provider.submitTx(tx);
    }

    /**
     * transfer between franklin
     * @param address 
     * @param token 
     * @param amount 
     * @param fee 
     */
    async transfer(address: Address, token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
        if (this.franklinState.pending_txs.length > 0) {
            return {
                err: "please wait for all pending transactions to complete before sending a new one."
            };
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
        await this.updateState();
        await this.fetchFranklinState();
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

    async fetchEthState() {
        let onchainBalances = new Array<string>(this.supportedTokens.length);
        let contractBalances = new Array<string>(this.supportedTokens.length);
        let lockedBlocksLeft = new Array<string>(this.supportedTokens.length);

        const currentBlock = await this.ethWallet.provider.getBlockNumber();

        const franklinDeployedContract = new Contract(process.env.CONTRACT_ADDR, franklinContractCode.interface, this.ethWallet);
        for(let token  of this.supportedTokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.getBalance().then(b => b.toString())
            } else {
                const erc20DeployedToken = new Contract(token.address, IERC20Conract.abi, this.ethWallet);
                onchainBalances[token.id] = await erc20DeployedToken.balanceOf(this.ethWallet.address).then(n => n.toString());
            }
            const balanceStorage = await franklinDeployedContract.balances(this.ethWallet.address, token.id);
            contractBalances[token.id] = balanceStorage.balance.toString();
            lockedBlocksLeft[token.id] = Math.max(balanceStorage.lockedUntilBlock - currentBlock, 0).toString();
        }

        this.ethState = {onchainBalances, contractBalances, lockedBlocksLeft};
    }

    async fetchFranklinState() {
        this.supportedTokens = await this.provider.getTokens();
        this.franklinState = await this.provider.getState(this.address);
    }

    // private state_ = null;
    // private state_timestamp_ = null;
    // private async state() {
    //     const update_interval = 1000;
    //     let curr_time = Date.now();
    //     if (this.state_timestamp_ === null || curr_time - this.state_timestamp_ > update_interval) {
    //         this.state_ = await this.getState();
    //         this.state_timestamp_ = curr_time;
    //     }
    //     return this.state_;
    // }

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
        return this.supportedTokens;
        // return [0, 1];
    }

    /**
     * get a list of balances in the mainchain
     */
    async getCommittedOnchainState() {
        let tokens = await this.getOnchainTokensList();
        let res = [];
        for (let t = 0; t < tokens.length; ++t) {
            let currToken = this.supportedTokens[t];
            let balance = (await this.getOnchainBalanceForToken(currToken)).toString(10);
            res.push({
                token: currToken.symbol,
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
    private async getCommittedContractTokensList() {
        // TODO:
        // this can be retrieved from our contract or our server
        return await this.getOnchainTokensList();
    }

    private async getContractBalanceForToken(token: Token) {
        let address = this.ethWallet.address;
        let [balance, block] = await this.contract.balances(address, token.id);
        let currBlock = await this.ethWallet.provider.getBlockNumber();
        let isLocked = currBlock < block; // ? 'locked' : 'unlocked';
        balance = balance.toString(10);
        return {
            balance,
            isLocked
        };
    }

    /**
     * locked/unlocked balances in our contract
     */
    async getCommittedContractBalances() {
        let tokens: Token[] = await this.getCommittedContractTokensList();
        let res = [];
        for (let i = 0; i < tokens.length; i++) {
            let token: Token = tokens[i];
            let balanceInfo = await this.getContractBalanceForToken(token);
            res.push({
                token: token.symbol,
                balance: balanceInfo.balance,
                isLocked: balanceInfo.isLocked
            });
        }
        return {
            contractBalances: res
        };
    }

    async getCommittedContractBalancesString() {
        return {
            contractBalances: (await this.getCommittedContractBalances()).contractBalances.map(balance => ({
                token: balance.token,
                balance: balance.balance.toString(10),
                isLocked: balance.isLocked
            }))
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
        let balances = this.franklinState.verified.balances;
        let tokens = this.supportedTokens;
        let res = [];
        for (let t = 0; t < tokens.length; t++) {
            let token = tokens[t];
            let balance = balances[t];
            res.push({
                token: token.symbol,
                balance: balance,
            });
        }
        return {
            balances: res
        };
    }

    /**
     * 
     */
    async getCommittedFranklinState() {
        let balances = this.franklinState.commited.balances;
        let tokens = this.supportedTokens;
        let res = [];
        for (let t = 0; t < tokens.length; t++) {
            let token = tokens[t];
            let balance = balances[t] || 0;
            res.push({
                token: token.symbol,
                balance: balance,
            });
        }
        return {
            balances: res
        };
    }
    
    /**
     * take verified state and here, on the client, apply transactions 
     * sent during this session, to get the pending state.
     */
    async getPendingFranklinState() {
        let committed = (await this.getCommittedFranklinState()).balances;
        // return {
        //     balances: committed
        // };
        
        let pendingTransactions = this.franklinState.pending_txs;
        for (let i = 0; i < pendingTransactions.length; i++) {
            let tx = pendingTransactions[i];
            for (let j = 0; j < committed.length; j++) {
                let balance = committed[j];
                let symbol = this.supportedTokens[tx.token].symbol
                
                let cmp1 = String(symbol);
                let cmp2 = String(balance.token);

                // console.log('symboll:', cmp1, cmp2, cmp1 === cmp2);

                let n1 = balance.balance;
                let n2 = tx.amount;

                // console.log('symboll3:', n1, n2);

                let add1 = bigNumberify(balance.balance);
                let add2 = bigNumberify(tx.amount);

                // console.log('symboll2:', add1, add2)

                if (cmp1 !== cmp2) continue;

                if (tx.to === this.address) {
                    balance.balance = bigNumberify(balance.balance).add(bigNumberify(tx.amount));
                } else if (tx.from === this.address) {
                    balance.balance = bigNumberify(balance.balance).sub(bigNumberify(tx.amount));
                } else {
                    throw new Error ('pending transactions must be related to this account.');
                }
            }
        }

        return {
            balances: committed
        };
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

    async updateState() {
        await this.fetchFranklinState();
        await this.fetchEthState();
    }

    async waitPendingTxsExecuted() {
        await this.fetchFranklinState();
        while (this.franklinState.pending_txs.length > 0) {
            await sleep(1000);
            await this.fetchFranklinState();
        }
    }
}

function sleep(ms) {
    return new Promise(resolve => {
        setTimeout(resolve, ms);
    });
}
