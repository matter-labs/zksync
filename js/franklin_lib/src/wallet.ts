import BN = require('bn.js');
import Axios from 'axios';
import { altjubjubCurve, pedersenHash } from './sign';
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import { HmacSHA512 } from 'crypto-js';
import 'ethers';
import {Contract, ethers} from 'ethers';
import {BigNumber, bigNumberify, BigNumberish, parseEther} from "ethers/utils";

const IERC20Conract = require("openzeppelin-solidity/build/contracts/IERC20");
import {franklinContractCode} from "../../../contracts/src.ts/deploy";

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
    address: Address;
    privateKey: BN;
    publicKey: EdwardsPoint;

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

    async widthdrawOffchain(token: Token, amount: BN, fee: BN) {
        let nonce = await this.getNonce();
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
        await this.fetchFranklinState();
        return this.franklinState.commited.nonce
    }

    static async fromEthWallet(wallet: ethers.Wallet) {
        let defaultFranklinProvider = new FranklinProvider();
        let seed = (await wallet.signMessage('Matter login')).substr(2);
        let frankinWallet = new Wallet(Buffer.from(seed, 'hex'), defaultFranklinProvider, wallet);
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

    async updateState() {
        await this.fetchFranklinState();
        await this.fetchEthState();
    }

    async waitPendingTxsExecuted() {
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
