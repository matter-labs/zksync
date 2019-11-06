import BN = require('bn.js');
import {
    privateKeyFromSeed,
    privateKeyToPublicKey,
    pubkeyToAddress,
    serializePointPacked,
    signTransactionBytes,
} from './crypto';
import { Contract, ContractTransaction, ethers, utils } from 'ethers';
import { packAmount, packFee } from './utils';
import { curve } from 'elliptic';
import {
    Address,
    CloseTx,
    DepositTx,
    ETHAccountState,
    FullExitReq,
    Nonce,
    SidechainAccountState,
    TokenLike,
    TransferTx,
    WithdrawTx,
} from './types';
import { SidechainProvider } from './provider';

const IERC20ConractInterface = new utils.Interface(require('../abi/IERC20.json').interface);
const sidechainMainContractInterface = new utils.Interface(require('../abi/Franklin.json').interface);
const priorityQueueInterface = new utils.Interface(require('../abi/PriorityQueue.json').interface);

export class Wallet {
    address: Address;
    walletKeys: WalletKeys;

    constructor(seed: Buffer, public provider: SidechainProvider, public ethWallet: ethers.Signer) {
        const { privateKey } = privateKeyFromSeed(seed);
        this.walletKeys = new WalletKeys(privateKey);
        this.address = `0x${pubkeyToAddress(this.walletKeys.publicKey).toString('hex')}`;
    }

    protected async depositETH(amount: utils.BigNumberish, maxFee: utils.BigNumberish) {
        const mainSidechainContract = new Contract(
            this.provider.sideChainInfo.contract_address,
            sidechainMainContractInterface,
            this.ethWallet,
        );
        return await mainSidechainContract.depositETH(amount, this.address, {
            value: utils.bigNumberify(amount).add(maxFee),
            gasLimit: utils.bigNumberify('200000'),
        });
    }

    protected async approveERC20(tokenLike: TokenLike, amount: utils.BigNumberish, options?: Object) {
        const token = await this.provider.resolveToken(tokenLike);
        const erc20contract = new Contract(token.address, IERC20ConractInterface, this.ethWallet);
        return await erc20contract.approve(this.provider.sideChainInfo.contract_address, amount, options);
    }

    protected async depositApprovedERC20(
        tokenLike: TokenLike,
        amount: utils.BigNumberish,
        maxEthFee: utils.BigNumberish,
        options?: Object,
    ) {
        const token = await this.provider.resolveToken(tokenLike);
        const mainSidechainContract = new Contract(
            this.provider.sideChainInfo.contract_address,
            sidechainMainContractInterface,
            this.ethWallet,
        );
        return await mainSidechainContract.depositERC20(
            token.address,
            amount,
            this.address,
            Object.assign({ gasLimit: utils.bigNumberify('250000'), value: maxEthFee }, options),
        );
    }

    async deposit(tokenLike: TokenLike, amount: utils.BigNumberish, maxEthFee: utils.BigNumberish) {
        const token = await this.provider.resolveToken(tokenLike);
        let contractTx;
        if (token.id === 0) {
            contractTx = await this.depositETH(amount, maxEthFee);
        } else {
            await this.approveERC20(token, amount);
            contractTx = await this.depositApprovedERC20(token, amount, maxEthFee);
        }
        return new DepositTransactionHandle(contractTx, { to: this.address, amount, token }, this.provider);
    }

    async withdrawFromContractToETHAddress(
        tokenLike: TokenLike,
        amount: utils.BigNumberish,
    ): Promise<ContractTransaction> {
        const token = await this.provider.resolveToken(tokenLike);
        const sidechainMainContract = new Contract(
            this.provider.sideChainInfo.contract_address,
            sidechainMainContractInterface,
            this.ethWallet,
        );
        if (token.id === 0) {
            return await sidechainMainContract.withdrawETH(amount, { gasLimit: 200000 });
        } else {
            return await sidechainMainContract.withdrawERC20(token.address, amount, {
                gasLimit: utils.bigNumberify('150000'),
            });
        }
    }

    async withdrawFromSidechainToContract(
        tokenLike: TokenLike,
        amount: utils.BigNumberish,
        fee: utils.BigNumberish,
        nonce: Nonce = 'commited',
    ): Promise<TransactionHandle> {
        const token = await this.provider.resolveToken(tokenLike);
        const tx = {
            account: this.address,
            eth_address: await this.ethWallet.getAddress(),
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(nonce),
        };
        const signature = this.walletKeys.signWithdraw(tx);
        const tx_req = SidechainProvider.prepareWithdrawTxForApi(tx, signature);

        const submitResponse = await this.provider.submitTx(tx_req);
        return new TransactionHandle(tx, submitResponse, this.provider);
    }

    async emergencyWithdraw(tokenLike: TokenLike, nonce: Nonce = 'commited') {
        const token = await this.provider.resolveToken(tokenLike);
        const sidechainMainContract = new Contract(
            this.provider.sideChainInfo.contract_address,
            sidechainMainContractInterface,
            this.ethWallet,
        );
        const nonceNumber = await this.getNonce(nonce);
        const signature = this.walletKeys.signFullExit({
            token: token.id,
            eth_address: await this.ethWallet.getAddress(),
            nonce: nonceNumber,
        });
        const tx = await sidechainMainContract.fullExit(
            serializePointPacked(this.walletKeys.publicKey),
            token.address,
            signature,
            nonceNumber,
            { gasLimit: utils.bigNumberify('500000'), value: utils.parseEther('0.02') },
        );
        return tx.hash;
    }

    async transfer(
        to: Address,
        tokenLike: TokenLike,
        amount: utils.BigNumberish,
        fee: utils.BigNumberish,
        nonce: Nonce = 'commited',
    ): Promise<TransactionHandle> {
        const token = await this.provider.resolveToken(tokenLike);
        const tx = {
            from: this.address,
            to,
            token: token.id,
            amount,
            fee,
            nonce: await this.getNonce(nonce),
        };
        const signature = this.walletKeys.signTransfer(tx);
        const tx_req = SidechainProvider.prepareTransferTxForApi(tx, signature);

        const submitResponse = await this.provider.submitTx(tx_req);
        return new TransactionHandle(tx, submitResponse, this.provider);
    }

    async close(): Promise<TransactionHandle> {
        const tx = {
            account: this.address,
            nonce: await this.getNonce(),
        };

        const signature = this.walletKeys.signClose(tx);
        const tx_req = SidechainProvider.prepareCloseRequestForApi(tx, signature);

        const submitResponse = await this.provider.submitTx(tx_req);
        return new TransactionHandle(tx, submitResponse, this.provider);
    }

    async getNonce(nonce: Nonce = 'commited'): Promise<number> {
        if (nonce == 'commited') {
            return (await this.provider.getState(this.address)).commited.nonce;
        } else if (typeof nonce == 'number') {
            return nonce;
        }
    }

    static async fromEthWallet(ethWallet: ethers.Signer, sidechainProvider: SidechainProvider) {
        const seed = (await ethWallet.signMessage('Matter login')).substr(2);
        return new Wallet(Buffer.from(seed, 'hex'), sidechainProvider, ethWallet);
    }

    async getETHBalances(): Promise<ETHAccountState> {
        const tokens = this.provider.sideChainInfo.tokens;
        const onchainBalances = new Array<utils.BigNumber>(tokens.length);
        const contractBalances = new Array<utils.BigNumber>(tokens.length);

        const sidechainMainContract = new Contract(
            this.provider.sideChainInfo.contract_address,
            sidechainMainContractInterface,
            this.ethWallet,
        );
        const ethAddress = await this.ethWallet.getAddress();
        for (const token of tokens) {
            if (token.id == 0) {
                onchainBalances[token.id] = await this.ethWallet.provider.getBalance(ethAddress);
            } else {
                const erc20token = new Contract(token.address, IERC20ConractInterface, this.ethWallet);
                onchainBalances[token.id] = await erc20token.balanceOf(ethAddress);
            }
            contractBalances[token.id] = await sidechainMainContract.balancesToWithdraw(ethAddress, token.id);
        }

        return { onchainBalances, contractBalances };
    }

    async getAccountState(): Promise<SidechainAccountState> {
        return this.provider.getState(this.address);
    }
}

export class WalletKeys {
    publicKey: curve.edwards.EdwardsPoint;

    constructor(public privateKey: BN) {
        this.publicKey = privateKeyToPublicKey(privateKey);
    }

    signTransfer(tx: TransferTx) {
        const type = Buffer.from([5]); // tx type
        const from = serializeAddress(tx.from);
        const to = serializeAddress(tx.to);
        const token = serializeTokenId(tx.token);
        const amount = serializeAmountPacked(tx.amount);
        const fee = serializeFeePacked(tx.fee);
        const nonce = serializeNonce(tx.nonce);
        const msg = Buffer.concat([type, from, to, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signWithdraw(tx: WithdrawTx) {
        const type = Buffer.from([3]);
        const account = serializeAddress(tx.account);
        const eth_address = serializeAddress(tx.eth_address);
        const token = serializeTokenId(tx.token);
        const amount = serializeAmountFull(tx.amount);
        const fee = serializeFeePacked(tx.fee);
        const nonce = serializeNonce(tx.nonce);
        const msg = Buffer.concat([type, account, eth_address, token, amount, fee, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signClose(tx: CloseTx) {
        const type = Buffer.from([4]);
        const account = serializeAddress(tx.account);
        const nonce = serializeNonce(tx.nonce);

        const msg = Buffer.concat([type, account, nonce]);
        return signTransactionBytes(this.privateKey, msg);
    }

    signFullExit(op: FullExitReq) {
        const type = Buffer.from([6]);
        const packed_pubkey = serializePointPacked(this.publicKey);
        const eth_address = serializeAddress(op.eth_address);
        const token = serializeTokenId(op.token);
        const nonce = serializeNonce(op.nonce);
        const msg = Buffer.concat([type, packed_pubkey, eth_address, token, nonce]);
        return Buffer.from(signTransactionBytes(this.privateKey, msg).sign, 'hex');
    }
}

// Franklin or eth address
function serializeAddress(address: Address | string): Buffer {
    return Buffer.from(address.substr(2), 'hex');
}
function serializeTokenId(tokenId: number): Buffer {
    const buffer = Buffer.alloc(2);
    buffer.writeUInt16BE(tokenId, 0);
    return buffer;
}

function serializeAmountPacked(amount: utils.BigNumberish): Buffer {
    const bnAmount = new BN(utils.bigNumberify(amount).toString());
    return packAmount(bnAmount);
}

function serializeAmountFull(amount: utils.BigNumberish): Buffer {
    const bnAmount = new BN(utils.bigNumberify(amount).toString());
    return bnAmount.toArrayLike(Buffer, 'be', 16);
}

function serializeFeePacked(fee: utils.BigNumberish): Buffer {
    const bnFee = new BN(utils.bigNumberify(fee).toString());
    return packFee(bnFee);
}

function serializeNonce(nonce: number): Buffer {
    const buff = Buffer.alloc(4);
    buff.writeUInt32BE(nonce, 0);
    return buff;
}

class DepositTransactionHandle {
    state: 'Sent' | 'Mined' | 'Commited' | 'Verified';
    priorityOpId?: utils.BigNumber;

    constructor(
        public ethTx: ContractTransaction,
        public depositTx: DepositTx,
        public sidechainProvider: SidechainProvider,
    ) {
        this.state = 'Sent';
    }

    async waitTxMine() {
        if (this.state != 'Sent') return;

        const txReceipt = await this.ethTx.wait();
        for (const log of txReceipt.logs) {
            const priorityQueueLog = priorityQueueInterface.parseLog(txReceipt.logs[0]);
            if (priorityQueueLog) {
                this.priorityOpId = priorityQueueLog.values.serialId;
            }
        }
        if (!this.priorityOpId) {
            throw new Error('Failed to parse tx logs');
        }

        this.state = 'Mined';
    }

    async waitCommit() {
        await this.waitTxMine();
        if (this.state != 'Mined') return;
        await this.sidechainProvider.notifyPriorityOp(this.priorityOpId.toNumber(), 'COMMIT');
        this.state = 'Commited';
    }

    async waitVerify() {
        await this.waitCommit();
        if (this.state != 'Commited') return;

        await this.sidechainProvider.notifyPriorityOp(this.priorityOpId.toNumber(), 'VERIFY');
        this.state = 'Verified';
    }
}

class TransactionHandle {
    state: 'Sent' | 'Commited' | 'Verified';

    constructor(public txData, public txHash: string, public sidechainProvider: SidechainProvider) {
        this.state = 'Sent';
    }

    async waitCommit() {
        if (this.state !== 'Sent') return;

        await this.sidechainProvider.notifyTransaction(this.txHash, 'COMMIT');
        this.state = 'Commited';
    }

    async waitVerify() {
        await this.waitCommit();
        await this.sidechainProvider.notifyTransaction(this.txHash, 'VERIFY');
        this.state = 'Verified';
    }
}
