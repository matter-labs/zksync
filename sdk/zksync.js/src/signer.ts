import { privateKeyFromSeed, signTransactionBytes, privateKeyToPubKeyHash } from './crypto';
import { BigNumber, BigNumberish, ethers } from 'ethers';
import {
    getEthSignatureType,
    signMessagePersonalAPI,
    getSignedBytesFromMessage,
    serializeAccountId,
    serializeAddress,
    serializeTokenId,
    serializeAmountPacked,
    serializeFeePacked,
    serializeNonce,
    serializeAmountFull,
    getCREATE2AddressAndSalt,
    serializeTimestamp
} from './utils';
import {
    Address,
    EthSignerType,
    PubKeyHash,
    Transfer,
    Withdraw,
    ForcedExit,
    ChangePubKey,
    ChangePubKeyOnchain,
    ChangePubKeyECDSA,
    ChangePubKeyCREATE2,
    ZkSyncVersion
} from './types';
import validate = WebAssembly.validate;

export class Signer {
    readonly #privateKey: Uint8Array;

    private constructor(privKey: Uint8Array) {
        this.#privateKey = privKey;
    }

    async pubKeyHash(): Promise<PubKeyHash> {
        return await privateKeyToPubKeyHash(this.#privateKey);
    }

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
        const type = new Uint8Array([5]); // tx type
        const accountId = serializeAccountId(transfer.accountId);
        const from = serializeAddress(transfer.from);
        const to = serializeAddress(transfer.to);
        const token = serializeTokenId(transfer.tokenId);
        const amount = serializeAmountPacked(transfer.amount);
        const fee = serializeFeePacked(transfer.fee);
        const nonce = serializeNonce(transfer.nonce);
        const validFrom = serializeTimestamp(transfer.validFrom);
        const validUntil = serializeTimestamp(transfer.validUntil);
        const msgBytes = ethers.utils.concat([
            type,
            accountId,
            from,
            to,
            token,
            amount,
            fee,
            nonce,
            validFrom,
            validUntil
        ]);

        return msgBytes;
    }

    async signSyncTransfer(
        transfer: {
            accountId: number;
            from: Address;
            to: Address;
            tokenId: number;
            amount: BigNumberish;
            fee: BigNumberish;
            nonce: number;
            validFrom: number;
            validUntil: number;
        },
        zkSyncVersion: ZkSyncVersion
    ): Promise<Transfer> {
        if (zkSyncVersion === 'contracts-3') {
            throw new Error('Contracts-3 version is not supported by this version of sdk');
        }
        const msgBytes = this.transferSignBytes(transfer);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            type: 'Transfer',
            accountId: transfer.accountId,
            from: transfer.from,
            to: transfer.to,
            token: transfer.tokenId,
            amount: BigNumber.from(transfer.amount).toString(),
            fee: BigNumber.from(transfer.fee).toString(),
            nonce: transfer.nonce,
            validFrom: transfer.validFrom,
            validUntil: transfer.validUntil,
            signature
        };
    }

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
        const typeBytes = new Uint8Array([3]);
        const accountId = serializeAccountId(withdraw.accountId);
        const accountBytes = serializeAddress(withdraw.from);
        const ethAddressBytes = serializeAddress(withdraw.ethAddress);
        const tokenIdBytes = serializeTokenId(withdraw.tokenId);
        const amountBytes = serializeAmountFull(withdraw.amount);
        const feeBytes = serializeFeePacked(withdraw.fee);
        const nonceBytes = serializeNonce(withdraw.nonce);
        const validFrom = serializeTimestamp(withdraw.validFrom);
        const validUntil = serializeTimestamp(withdraw.validUntil);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            accountId,
            accountBytes,
            ethAddressBytes,
            tokenIdBytes,
            amountBytes,
            feeBytes,
            nonceBytes,
            validFrom,
            validUntil
        ]);

        return msgBytes;
    }

    async signSyncWithdraw(
        withdraw: {
            accountId: number;
            from: Address;
            ethAddress: string;
            tokenId: number;
            amount: BigNumberish;
            fee: BigNumberish;
            nonce: number;
            validFrom: number;
            validUntil: number;
        },
        zkSyncVersion: ZkSyncVersion
    ): Promise<Withdraw> {
        if (zkSyncVersion === 'contracts-3') {
            throw new Error('Contracts-3 version is not supported by this version of sdk');
        }
        const msgBytes = this.withdrawSignBytes(withdraw);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);

        return {
            type: 'Withdraw',
            accountId: withdraw.accountId,
            from: withdraw.from,
            to: withdraw.ethAddress,
            token: withdraw.tokenId,
            amount: BigNumber.from(withdraw.amount).toString(),
            fee: BigNumber.from(withdraw.fee).toString(),
            nonce: withdraw.nonce,
            validFrom: withdraw.validFrom,
            validUntil: withdraw.validUntil,
            signature
        };
    }

    forcedExitSignBytes(forcedExit: {
        initiatorAccountId: number;
        target: Address;
        tokenId: number;
        fee: BigNumberish;
        nonce: number;
        validFrom: number;
        validUntil: number;
    }): Uint8Array {
        const typeBytes = new Uint8Array([8]);
        const initiatorAccountIdBytes = serializeAccountId(forcedExit.initiatorAccountId);
        const targetBytes = serializeAddress(forcedExit.target);
        const tokenIdBytes = serializeTokenId(forcedExit.tokenId);
        const feeBytes = serializeFeePacked(forcedExit.fee);
        const nonceBytes = serializeNonce(forcedExit.nonce);
        const validFrom = serializeTimestamp(forcedExit.validFrom);
        const validUntil = serializeTimestamp(forcedExit.validUntil);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            initiatorAccountIdBytes,
            targetBytes,
            tokenIdBytes,
            feeBytes,
            nonceBytes,
            validFrom,
            validUntil
        ]);

        return msgBytes;
    }

    async signSyncForcedExit(
        forcedExit: {
            initiatorAccountId: number;
            target: Address;
            tokenId: number;
            fee: BigNumberish;
            nonce: number;
            validFrom: number;
            validUntil: number;
        },
        zkSyncVersion: ZkSyncVersion
    ): Promise<ForcedExit> {
        if (zkSyncVersion === 'contracts-3') {
            throw new Error('Contracts-3 version is not supported by this version of sdk');
        }
        const msgBytes = this.forcedExitSignBytes(forcedExit);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);
        return {
            type: 'ForcedExit',
            initiatorAccountId: forcedExit.initiatorAccountId,
            target: forcedExit.target,
            token: forcedExit.tokenId,
            fee: BigNumber.from(forcedExit.fee).toString(),
            nonce: forcedExit.nonce,
            validFrom: forcedExit.validFrom,
            validUntil: forcedExit.validUntil,
            signature
        };
    }

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
        const typeBytes = new Uint8Array([7]); // Tx type (1 byte)
        const accountIdBytes = serializeAccountId(changePubKey.accountId);
        const accountBytes = serializeAddress(changePubKey.account);
        const pubKeyHashBytes = serializeAddress(changePubKey.newPkHash);
        const tokenIdBytes = serializeTokenId(changePubKey.feeTokenId);
        const feeBytes = serializeFeePacked(changePubKey.fee);
        const nonceBytes = serializeNonce(changePubKey.nonce);
        const validFrom = serializeTimestamp(changePubKey.validFrom);
        const validUntil = serializeTimestamp(changePubKey.validUntil);
        const msgBytes = ethers.utils.concat([
            typeBytes,
            accountIdBytes,
            accountBytes,
            pubKeyHashBytes,
            tokenIdBytes,
            feeBytes,
            nonceBytes,
            validFrom,
            validUntil
        ]);

        return msgBytes;
    }

    async signSyncChangePubKey(
        changePubKey: {
            accountId: number;
            account: Address;
            newPkHash: PubKeyHash;
            feeTokenId: number;
            fee: BigNumberish;
            nonce: number;
            ethAuthData: ChangePubKeyOnchain | ChangePubKeyECDSA | ChangePubKeyCREATE2;
            validFrom: number;
            validUntil: number;
        },
        zkSyncVersion: ZkSyncVersion
    ): Promise<ChangePubKey> {
        if (zkSyncVersion === 'contracts-3') {
            throw new Error('Contracts-3 version is not supported by this version of sdk');
        }
        const msgBytes = this.changePubKeySignBytes(changePubKey);
        const signature = await signTransactionBytes(this.#privateKey, msgBytes);
        return {
            type: 'ChangePubKey',
            accountId: changePubKey.accountId,
            account: changePubKey.account,
            newPkHash: changePubKey.newPkHash,
            feeToken: changePubKey.feeTokenId,
            fee: BigNumber.from(changePubKey.fee).toString(),
            nonce: changePubKey.nonce,
            signature,
            ethAuthData: changePubKey.ethAuthData,
            validFrom: changePubKey.validFrom,
            validUntil: changePubKey.validUntil
        };
    }

    static fromPrivateKey(pk: Uint8Array): Signer {
        return new Signer(pk);
    }

    static async fromSeed(seed: Uint8Array): Promise<Signer> {
        return new Signer(await privateKeyFromSeed(seed));
    }

    static async fromETHSignature(
        ethSigner: ethers.Signer
    ): Promise<{
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
        const signedBytes = getSignedBytesFromMessage(message, false);
        const signature = await signMessagePersonalAPI(ethSigner, signedBytes);
        const address = await ethSigner.getAddress();
        const ethSignatureType = await getEthSignatureType(ethSigner.provider, message, signature, address);
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
        public create2WalletData: {
            creatorAddress: string;
            saltArg: string;
            codeHash: string;
        },
        provider?: ethers.providers.Provider
    ) {
        super();
        Object.defineProperty(this, 'provider', {
            enumerable: true,
            value: provider,
            writable: false
        });
        const create2Info = getCREATE2AddressAndSalt(zkSyncPubkeyHash, create2WalletData);
        this.address = create2Info.address;
        this.salt = create2Info.salt;
    }

    async getAddress() {
        return this.address;
    }

    /**
     * This signer can't sign messages but we return zeroed signature bytes to comply with zksync API for now.
     */
    async signMessage(_message) {
        return '0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000';
    }

    async signTransaction(_message): Promise<string> {
        throw new Error("Create2Wallet signer can't sign transactions");
    }

    connect(provider: ethers.providers.Provider): ethers.Signer {
        return new Create2WalletSigner(this.zkSyncPubkeyHash, this.create2WalletData, provider);
    }
}
