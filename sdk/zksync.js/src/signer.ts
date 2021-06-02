import { privateKeyFromSeed, signTransactionBytes, privateKeyToPubKeyHash } from './crypto';
import { BigNumber, BigNumberish, ethers } from 'ethers';
import * as utils from './utils';
import {
    Address,
    EthSignerType,
    PubKeyHash,
    Transfer,
    Withdraw,
    ForcedExit,
    ChangePubKey,
    MintNFT,
    WithdrawNFT,
    ChangePubKeyOnchain,
    ChangePubKeyECDSA,
    ChangePubKeyCREATE2,
    Create2Data,
    Swap,
    Order,
    Ratio
} from './types';

export class Signer {
    readonly #privateKey: Uint8Array;

    private constructor(privKey: Uint8Array) {
        this.#privateKey = privKey;
    }

    async pubKeyHash(): Promise<PubKeyHash> {
        return await privateKeyToPubKeyHash(this.#privateKey);
    }

    async signMintNFT(mintNft: {
        creatorId: number;
        creatorAddress: Address;
        recipient: Address;
        contentHash: string;
        feeTokenId: number;
        fee: BigNumberish;
        nonce: number;
    }): Promise<MintNFT> {
        const tx: MintNFT = {
            ...mintNft,
            type: 'MintNFT',
            feeToken: mintNft.feeTokenId
        };
        const msgBytes = utils.serializeMintNFT(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...tx,
            fee: BigNumber.from(mintNft.fee).toString(),
            signature
        };
    }

    async signWithdrawNFT(withdrawNft: {
        accountId: number;
        from: Address;
        to: Address;
        tokenId: number;
        feeTokenId: number;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<WithdrawNFT> {
        const tx: WithdrawNFT = {
            ...withdrawNft,
            type: 'WithdrawNFT',
            token: withdrawNft.tokenId,
            feeToken: withdrawNft.feeTokenId
        };
        const msgBytes = utils.serializeWithdrawNFT(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...tx,
            fee: BigNumber.from(withdrawNft.fee).toString(),
            signature
        };
    }

    /**
     * @deprecated `Signer.*SignBytes` methods will be removed in future. Use `utils.serializeTx` instead.
     */
    transferSignBytes(transfer: {
        accountId: number;
        from: Address;
        to: Address;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Uint8Array {
        return utils.serializeTransfer({
            ...transfer,
            type: 'Transfer',
            token: transfer.tokenId
        });
    }

    async signSyncOrder(order: Order): Promise<Order> {
        const msgBytes = utils.serializeOrder(order);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...order,
            amount: BigNumber.from(order.amount).toString(),
            ratio: order.ratio.map((p) => BigNumber.from(p).toString()) as Ratio,
            signature
        };
    }

    async signSyncSwap(swap: {
        orders: [Order, Order];
        amounts: [BigNumberish, BigNumberish];
        submitterId: number;
        submitterAddress: Address;
        nonce: number;
        feeToken: number;
        fee: BigNumberish;
    }): Promise<Swap> {
        const tx: Swap = {
            ...swap,
            type: 'Swap'
        };

        const msgBytes = await utils.serializeSwap(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...tx,
            amounts: [BigNumber.from(tx.amounts[0]).toString(), BigNumber.from(tx.amounts[1]).toString()],
            fee: BigNumber.from(tx.fee).toString(),
            signature
        };
    }

    async signSyncTransfer(transfer: {
        accountId: number;
        from: Address;
        to: Address;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Transfer> {
        const tx: Transfer = {
            ...transfer,
            type: 'Transfer',
            token: transfer.tokenId
        };
        const msgBytes = utils.serializeTransfer(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...tx,
            amount: BigNumber.from(transfer.amount).toString(),
            fee: BigNumber.from(transfer.fee).toString(),
            signature
        };
    }

    /**
     * @deprecated `Signer.*SignBytes` methods will be removed in future. Use `utils.serializeTx` instead.
     */
    withdrawSignBytes(withdraw: {
        accountId: number;
        from: Address;
        ethAddress: string;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Uint8Array {
        return utils.serializeWithdraw({
            ...withdraw,
            type: 'Withdraw',
            to: withdraw.ethAddress,
            token: withdraw.tokenId
        });
    }

    async signSyncWithdraw(withdraw: {
        accountId: number;
        from: Address;
        ethAddress: string;
        tokenId: number;
        amount: BigNumberish;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<Withdraw> {
        const tx: Withdraw = {
            ...withdraw,
            type: 'Withdraw',
            to: withdraw.ethAddress,
            token: withdraw.tokenId
        };
        const msgBytes = utils.serializeWithdraw(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            ...tx,
            amount: BigNumber.from(withdraw.amount).toString(),
            fee: BigNumber.from(withdraw.fee).toString(),
            signature
        };
    }

    /**
     * @deprecated `Signer.*SignBytes` methods will be removed in future. Use `utils.serializeTx` instead.
     */
    forcedExitSignBytes(forcedExit: {
        initiatorAccountId: number;
        target: Address;
        tokenId: number;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Uint8Array {
        return utils.serializeForcedExit({
            ...forcedExit,
            type: 'ForcedExit',
            token: forcedExit.tokenId
        });
    }

    async signSyncForcedExit(forcedExit: {
        initiatorAccountId: number;
        target: Address;
        tokenId: number;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Promise<ForcedExit> {
        const tx: ForcedExit = {
            ...forcedExit,
            type: 'ForcedExit',
            token: forcedExit.tokenId
        };
        const msgBytes = utils.serializeForcedExit(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);
        return {
            ...tx,
            fee: BigNumber.from(forcedExit.fee).toString(),
            signature
        };
    }

    /**
     * @deprecated `Signer.*SignBytes` methods will be removed in future. Use `utils.serializeTx` instead.
     */
    changePubKeySignBytes(changePubKey: {
        accountId: number;
        account: Address;
        newPkHash: PubKeyHash;
        feeTokenId: number;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Uint8Array {
        return utils.serializeChangePubKey({
            ...changePubKey,
            type: 'ChangePubKey',
            feeToken: changePubKey.feeTokenId,
            // this is not important for serialization
            ethAuthData: { type: 'Onchain' }
        });
    }

    async signSyncChangePubKey(changePubKey: {
        accountId: number;
        account: Address;
        newPkHash: PubKeyHash;
        feeTokenId: number;
        fee: BigNumberish;
        nonce: number;
        ethAuthData?: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
        ethSignature?: string;
        validFrom: number;
        validUntil: number;
    }): Promise<ChangePubKey> {
        const tx: ChangePubKey = {
            ...changePubKey,
            type: 'ChangePubKey',
            feeToken: changePubKey.feeTokenId
        };
        const msgBytes = utils.serializeChangePubKey(tx);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);
        return {
            ...tx,
            fee: BigNumber.from(changePubKey.fee).toString(),
            signature
        };
    }

    static fromPrivateKey(pk: Uint8Array): Signer {
        return new Signer(pk);
    }

    static async fromSeed(seed: Uint8Array): Promise<Signer> {
        return new Signer(await privateKeyFromSeed(seed));
    }

    static async fromETHSignature(ethSigner: ethers.Signer): Promise<{
        signer: Signer;
        ethSignatureType: EthSignerType;
    }> {
        let chainID = 1;
        if (ethSigner.provider) {
            const network = await ethSigner.provider.getNetwork();
            chainID = network.chainId;
        }
        let message = 'Access zkSync account.\n\nOnly sign this message for a trusted client!';
        if (chainID !== 1) {
            message += `\nChain ID: ${chainID}.`;
        }
        const signedBytes = utils.getSignedBytesFromMessage(message, false);
        const signature = await utils.signMessagePersonalAPI(ethSigner, signedBytes);
        const address = await ethSigner.getAddress();
        const ethSignatureType = await utils.getEthSignatureType(ethSigner.provider, message, signature, address);
        const seed = ethers.utils.arrayify(signature);
        const signer = await Signer.fromSeed(seed);
        return { signer, ethSignatureType };
    }
}

export class Create2WalletSigner extends ethers.Signer {
    public readonly address: string;
    // salt for create2 function call
    public readonly salt: string;
    constructor(
        public zkSyncPubkeyHash: string,
        public create2WalletData: Create2Data,
        provider?: ethers.providers.Provider
    ) {
        super();
        Object.defineProperty(this, 'provider', {
            enumerable: true,
            value: provider,
            writable: false
        });
        const create2Info = utils.getCREATE2AddressAndSalt(zkSyncPubkeyHash, create2WalletData);
        this.address = create2Info.address;
        this.salt = create2Info.salt;
    }

    async getAddress() {
        return this.address;
    }

    /**
     * This signer can't sign messages but we return zeroed signature bytes to comply with ethers API.
     */
    async signMessage(_message) {
        return ethers.utils.hexlify(new Uint8Array(65));
    }

    async signTransaction(_message): Promise<string> {
        throw new Error("Create2Wallet signer can't sign transactions");
    }

    connect(provider: ethers.providers.Provider): ethers.Signer {
        return new Create2WalletSigner(this.zkSyncPubkeyHash, this.create2WalletData, provider);
    }
}
