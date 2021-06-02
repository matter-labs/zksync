import { utils, constants, ethers, BigNumber, BigNumberish, Contract } from 'ethers';
import { Provider } from '.';
import {
    PubKeyHash,
    TokenAddress,
    TokenLike,
    Tokens,
    TokenSymbol,
    EthSignerType,
    Address,
    Transfer,
    ForcedExit,
    ChangePubKey,
    Withdraw,
    CloseAccount,
    MintNFT,
    Order,
    Swap,
    TokenRatio,
    WeiRatio,
    WithdrawNFT
} from './types';
import { rescueHashOrders } from './crypto';

// Max number of tokens for the current version, it is determined by the zkSync circuit implementation.
const MAX_NUMBER_OF_TOKENS = Math.pow(2, 31);
// Max number of accounts for the current version, it is determined by the zkSync circuit implementation.
const MAX_NUMBER_OF_ACCOUNTS = Math.pow(2, 24);

export const MAX_TIMESTAMP = 4294967295;
export const MIN_NFT_TOKEN_ID = 65536;
export const CURRENT_TX_VERSION = 1;

export const IERC20_INTERFACE = new utils.Interface(require('../abi/IERC20.json').abi);
export const SYNC_MAIN_CONTRACT_INTERFACE = new utils.Interface(require('../abi/SyncMain.json').abi);
export const SYNC_GOV_CONTRACT_INTERFACE = new utils.Interface(require('../abi/SyncGov.json').abi);
export const SYNC_NFT_FACTORY_INTERFACE = new utils.Interface(require('../abi/NFTFactory.json').abi);
export const IEIP1271_INTERFACE = new utils.Interface(require('../abi/IEIP1271.json').abi);
export const MULTICALL_INTERFACE = new utils.Interface(require('../abi/Multicall.json').abi);

export const ERC20_DEPOSIT_GAS_LIMIT = require('../misc/DepositERC20GasLimit.json');

export const MAX_ERC20_APPROVE_AMOUNT = BigNumber.from(
    '115792089237316195423570985008687907853269984665640564039457584007913129639935'
); // 2^256 - 1

export const ERC20_APPROVE_TRESHOLD = BigNumber.from(
    '57896044618658097711785492504343953926634992332820282019728792003956564819968'
); // 2^255

// Gas limit that is set for eth deposit by default. For default EOA accounts 60k should be enough, but we reserve
// more gas for smart-contract wallets
export const ETH_RECOMMENDED_DEPOSIT_GAS_LIMIT = BigNumber.from('90000'); // 90k
// For normal wallet/erc20 token 90k gas for deposit should be enough, but for some tokens this can go as high as ~200k
// we try to be safe by default
export const ERC20_RECOMMENDED_DEPOSIT_GAS_LIMIT = BigNumber.from('300000'); // 300k

const AMOUNT_EXPONENT_BIT_WIDTH = 5;
const AMOUNT_MANTISSA_BIT_WIDTH = 35;
const FEE_EXPONENT_BIT_WIDTH = 5;
const FEE_MANTISSA_BIT_WIDTH = 11;

export function tokenRatio(ratio: { [token: string]: string | number; [token: number]: string | number }): TokenRatio {
    return {
        type: 'Token',
        ...ratio
    };
}

export function weiRatio(ratio: { [token: string]: BigNumberish; [token: number]: BigNumberish }): WeiRatio {
    return {
        type: 'Wei',
        ...ratio
    };
}

export function floatToInteger(
    floatBytes: Uint8Array,
    expBits: number,
    mantissaBits: number,
    expBaseNumber: number
): BigNumber {
    if (floatBytes.length * 8 !== mantissaBits + expBits) {
        throw new Error('Float unpacking, incorrect input length');
    }

    const bits = buffer2bitsBE(floatBytes).reverse();
    let exponent = BigNumber.from(0);
    let expPow2 = BigNumber.from(1);
    for (let i = 0; i < expBits; i++) {
        if (bits[i] === 1) {
            exponent = exponent.add(expPow2);
        }
        expPow2 = expPow2.mul(2);
    }
    exponent = BigNumber.from(expBaseNumber).pow(exponent);

    let mantissa = BigNumber.from(0);
    let mantissaPow2 = BigNumber.from(1);
    for (let i = expBits; i < expBits + mantissaBits; i++) {
        if (bits[i] === 1) {
            mantissa = mantissa.add(mantissaPow2);
        }
        mantissaPow2 = mantissaPow2.mul(2);
    }
    return exponent.mul(mantissa);
}

export function bitsIntoBytesInBEOrder(bits: number[]): Uint8Array {
    if (bits.length % 8 !== 0) {
        throw new Error('wrong number of bits to pack');
    }
    const nBytes = bits.length / 8;
    const resultBytes = new Uint8Array(nBytes);

    for (let byte = 0; byte < nBytes; ++byte) {
        let value = 0;
        if (bits[byte * 8] === 1) {
            value |= 0x80;
        }
        if (bits[byte * 8 + 1] === 1) {
            value |= 0x40;
        }
        if (bits[byte * 8 + 2] === 1) {
            value |= 0x20;
        }
        if (bits[byte * 8 + 3] === 1) {
            value |= 0x10;
        }
        if (bits[byte * 8 + 4] === 1) {
            value |= 0x08;
        }
        if (bits[byte * 8 + 5] === 1) {
            value |= 0x04;
        }
        if (bits[byte * 8 + 6] === 1) {
            value |= 0x02;
        }
        if (bits[byte * 8 + 7] === 1) {
            value |= 0x01;
        }

        resultBytes[byte] = value;
    }

    return resultBytes;
}

function numberToBits(integer: number, bits: number): number[] {
    const result = [];
    for (let i = 0; i < bits; i++) {
        result.push(integer & 1);
        integer /= 2;
    }
    return result;
}

export function integerToFloat(integer: BigNumber, expBits: number, mantissaBits: number, expBase: number): Uint8Array {
    const maxExponentPower = BigNumber.from(2).pow(expBits).sub(1);
    const maxExponent = BigNumber.from(expBase).pow(maxExponentPower);
    const maxMantissa = BigNumber.from(2).pow(mantissaBits).sub(1);

    if (integer.gt(maxMantissa.mul(maxExponent))) {
        throw new Error('Integer is too big');
    }

    // The algortihm is as follows: calculate minimal exponent
    // such that integer <= max_mantissa * exponent_base ^ exponent,
    // then if this minimal exponent is 0 we can choose mantissa equals integer and exponent equals 0
    // else we need to check two variants:
    // 1) with that minimal exponent
    // 2) with that minimal exponent minus 1
    let exponent = 0;
    let exponentTemp = BigNumber.from(1);
    while (integer.gt(maxMantissa.mul(exponentTemp))) {
        exponentTemp = exponentTemp.mul(expBase);
        exponent += 1;
    }
    let mantissa = integer.div(exponentTemp);
    if (exponent !== 0) {
        const variant1 = exponentTemp.mul(mantissa);
        const variant2 = exponentTemp.div(expBase).mul(maxMantissa);
        const diff1 = integer.sub(variant1);
        const diff2 = integer.sub(variant2);
        if (diff2.lt(diff1)) {
            mantissa = maxMantissa;
            exponent -= 1;
        }
    }

    // encode into bits. First bits of mantissa in LE order
    const encoding = [];

    encoding.push(...numberToBits(exponent, expBits));
    const mantissaNumber = mantissa.toNumber();
    encoding.push(...numberToBits(mantissaNumber, mantissaBits));

    return bitsIntoBytesInBEOrder(encoding.reverse()).reverse();
}

export function integerToFloatUp(
    integer: BigNumber,
    expBits: number,
    mantissaBits: number,
    expBase: number
): Uint8Array {
    const maxExponentPower = BigNumber.from(2).pow(expBits).sub(1);
    const maxExponent = BigNumber.from(expBase).pow(maxExponentPower);
    const maxMantissa = BigNumber.from(2).pow(mantissaBits).sub(1);

    if (integer.gt(maxMantissa.mul(maxExponent))) {
        throw new Error('Integer is too big');
    }

    // The algortihm is as follows: calculate minimal exponent
    // such that integer <= max_mantissa * exponent_base ^ exponent,
    // then mantissa is calculated as integer divided by exponent_base ^ exponent and rounded up
    let exponent = 0;
    let exponentTemp = BigNumber.from(1);
    while (integer.gt(maxMantissa.mul(exponentTemp))) {
        exponentTemp = exponentTemp.mul(expBase);
        exponent += 1;
    }
    let mantissa = integer.div(exponentTemp);
    if (!integer.mod(exponentTemp).eq(BigNumber.from(0))) {
        mantissa = mantissa.add(1);
    }

    // encode into bits. First bits of mantissa in LE order
    const encoding = [];

    encoding.push(...numberToBits(exponent, expBits));
    const mantissaNumber = mantissa.toNumber();
    encoding.push(...numberToBits(mantissaNumber, mantissaBits));

    return bitsIntoBytesInBEOrder(encoding.reverse()).reverse();
}

export function reverseBits(buffer: Uint8Array): Uint8Array {
    const reversed = buffer.reverse();
    reversed.map((b) => {
        // reverse bits in byte
        b = ((b & 0xf0) >> 4) | ((b & 0x0f) << 4);
        b = ((b & 0xcc) >> 2) | ((b & 0x33) << 2);
        b = ((b & 0xaa) >> 1) | ((b & 0x55) << 1);
        return b;
    });
    return reversed;
}

function packAmount(amount: BigNumber): Uint8Array {
    return reverseBits(integerToFloat(amount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10));
}

function packAmountUp(amount: BigNumber): Uint8Array {
    return reverseBits(integerToFloatUp(amount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10));
}

function packFee(amount: BigNumber): Uint8Array {
    return reverseBits(integerToFloat(amount, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10));
}

function packFeeUp(amount: BigNumber): Uint8Array {
    return reverseBits(integerToFloatUp(amount, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10));
}

export function packAmountChecked(amount: BigNumber): Uint8Array {
    if (closestPackableTransactionAmount(amount.toString()).toString() !== amount.toString()) {
        throw new Error('Transaction Amount is not packable');
    }
    return packAmount(amount);
}

export function packFeeChecked(amount: BigNumber): Uint8Array {
    if (closestPackableTransactionFee(amount.toString()).toString() !== amount.toString()) {
        throw new Error('Fee Amount is not packable');
    }
    return packFee(amount);
}

/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param amount
 */
export function closestPackableTransactionAmount(amount: BigNumberish): BigNumber {
    const packedAmount = packAmount(BigNumber.from(amount));
    return floatToInteger(packedAmount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10);
}

export function closestGreaterOrEqPackableTransactionAmount(amount: BigNumberish): BigNumber {
    const packedAmount = packAmountUp(BigNumber.from(amount));
    return floatToInteger(packedAmount, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, 10);
}

export function isTransactionAmountPackable(amount: BigNumberish): boolean {
    return closestPackableTransactionAmount(amount).eq(amount);
}

/**
 * packs and unpacks the amount, returning the closest packed value.
 * e.g 1000000003 => 1000000000
 * @param fee
 */
export function closestPackableTransactionFee(fee: BigNumberish): BigNumber {
    const packedFee = packFee(BigNumber.from(fee));
    return floatToInteger(packedFee, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10);
}

export function closestGreaterOrEqPackableTransactionFee(fee: BigNumberish): BigNumber {
    const packedFee = packFeeUp(BigNumber.from(fee));
    return floatToInteger(packedFee, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10);
}

export function isTransactionFeePackable(amount: BigNumberish): boolean {
    return closestPackableTransactionFee(amount).eq(amount);
}

// Check that this token could be an NFT.
// NFT is not represented in TokenSets, so we cannot check the availability of NFT in TokenSets
export function isNFT(token: TokenLike): boolean {
    return typeof token === 'number' && token >= MIN_NFT_TOKEN_ID;
}

export function buffer2bitsBE(buff) {
    const res = new Array(buff.length * 8);
    for (let i = 0; i < buff.length; i++) {
        const b = buff[i];
        res[i * 8] = (b & 0x80) !== 0 ? 1 : 0;
        res[i * 8 + 1] = (b & 0x40) !== 0 ? 1 : 0;
        res[i * 8 + 2] = (b & 0x20) !== 0 ? 1 : 0;
        res[i * 8 + 3] = (b & 0x10) !== 0 ? 1 : 0;
        res[i * 8 + 4] = (b & 0x08) !== 0 ? 1 : 0;
        res[i * 8 + 5] = (b & 0x04) !== 0 ? 1 : 0;
        res[i * 8 + 6] = (b & 0x02) !== 0 ? 1 : 0;
        res[i * 8 + 7] = (b & 0x01) !== 0 ? 1 : 0;
    }
    return res;
}

export function sleep(ms: number) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export function isTokenETH(token: TokenLike): boolean {
    return token === 'ETH' || token === constants.AddressZero;
}

type TokenOrId = TokenLike | number;

export class TokenSet {
    // TODO: handle stale entries, edge case when we rename token after adding it (ZKS-120).
    constructor(private tokensBySymbol: Tokens) {}

    private resolveTokenObject(tokenLike: TokenOrId) {
        if (this.tokensBySymbol[tokenLike]) {
            return this.tokensBySymbol[tokenLike];
        }

        for (const token of Object.values(this.tokensBySymbol)) {
            if (typeof tokenLike === 'number') {
                if (token.id === tokenLike) {
                    return token;
                }
            } else if (
                token.address.toLocaleLowerCase() === tokenLike.toLocaleLowerCase() ||
                token.symbol.toLocaleLowerCase() === tokenLike.toLocaleLowerCase()
            ) {
                return token;
            }
        }

        throw new Error(`Token ${tokenLike} is not supported`);
    }

    public isTokenTransferAmountPackable(tokenLike: TokenOrId, amount: string): boolean {
        const parsedAmount = this.parseToken(tokenLike, amount);
        return isTransactionAmountPackable(parsedAmount);
    }

    public isTokenTransactionFeePackable(tokenLike: TokenOrId, amount: string): boolean {
        const parsedAmount = this.parseToken(tokenLike, amount);
        return isTransactionFeePackable(parsedAmount);
    }

    public formatToken(tokenLike: TokenOrId, amount: BigNumberish): string {
        const decimals = this.resolveTokenDecimals(tokenLike);
        return utils.formatUnits(amount, decimals);
    }

    public parseToken(tokenLike: TokenOrId, amount: string): BigNumber {
        const decimals = this.resolveTokenDecimals(tokenLike);
        return utils.parseUnits(amount, decimals);
    }

    public resolveTokenDecimals(tokenLike: TokenOrId): number {
        if (isNFT(tokenLike)) {
            return 0;
        }
        return this.resolveTokenObject(tokenLike).decimals;
    }

    public resolveTokenId(tokenLike: TokenOrId): number {
        if (isNFT(tokenLike)) {
            return tokenLike as number;
        }
        return this.resolveTokenObject(tokenLike).id;
    }

    public resolveTokenAddress(tokenLike: TokenOrId): TokenAddress {
        return this.resolveTokenObject(tokenLike).address;
    }

    public resolveTokenSymbol(tokenLike: TokenOrId): TokenSymbol {
        return this.resolveTokenObject(tokenLike).symbol;
    }
}

export function getChangePubkeyMessage(
    pubKeyHash: PubKeyHash,
    nonce: number,
    accountId: number,
    batchHash?: string
): Uint8Array {
    const msgBatchHash = batchHash == undefined ? new Uint8Array(32).fill(0) : ethers.utils.arrayify(batchHash);
    const msgNonce = serializeNonce(nonce);
    const msgAccId = serializeAccountId(accountId);
    const msgPubKeyHash = serializeAddress(pubKeyHash);
    return ethers.utils.concat([msgPubKeyHash, msgNonce, msgAccId, msgBatchHash]);
}

export function getChangePubkeyLegacyMessage(pubKeyHash: PubKeyHash, nonce: number, accountId: number): Uint8Array {
    const msgNonce = utils.hexlify(serializeNonce(nonce));
    const msgAccId = utils.hexlify(serializeAccountId(accountId));
    const msgPubKeyHash = utils.hexlify(serializeAddress(pubKeyHash)).substr(2);
    const message =
        `Register zkSync pubkey:\n\n` +
        `${msgPubKeyHash}\n` +
        `nonce: ${msgNonce}\n` +
        `account id: ${msgAccId}\n\n` +
        `Only sign this message for a trusted client!`;
    return utils.toUtf8Bytes(message);
}

export function getSignedBytesFromMessage(message: utils.BytesLike | string, addPrefix: boolean): Uint8Array {
    let messageBytes = typeof message === 'string' ? utils.toUtf8Bytes(message) : utils.arrayify(message);
    if (addPrefix) {
        messageBytes = utils.concat([
            utils.toUtf8Bytes(`\x19Ethereum Signed Message:\n${messageBytes.length}`),
            messageBytes
        ]);
    }
    return messageBytes;
}

export async function signMessagePersonalAPI(signer: ethers.Signer, message: Uint8Array): Promise<string> {
    if (signer instanceof ethers.providers.JsonRpcSigner) {
        return signer.provider.send('personal_sign', [utils.hexlify(message), await signer.getAddress()]).then(
            (sign) => sign,
            (err) => {
                // We check for method name in the error string because error messages about invalid method name
                // often contain method name.
                if (err.message.includes('personal_sign')) {
                    // If no "personal_sign", use "eth_sign"
                    return signer.signMessage(message);
                }
                throw err;
            }
        );
    } else {
        return signer.signMessage(message);
    }
}

export async function verifyERC1271Signature(
    address: string,
    message: Uint8Array,
    signature: string,
    signerOrProvider: ethers.Signer | ethers.providers.Provider
): Promise<boolean> {
    const EIP1271_SUCCESS_VALUE = '0x1626ba7e';

    const signMessage = getSignedBytesFromMessage(message, true);
    const signMessageHash = utils.keccak256(signMessage);

    const eip1271 = new ethers.Contract(address, IEIP1271_INTERFACE, signerOrProvider);
    const eipRetVal = await eip1271.isValidSignature(signMessageHash, signature);
    return eipRetVal === EIP1271_SUCCESS_VALUE;
}

export async function getEthSignatureType(
    _provider: ethers.providers.Provider,
    message: string,
    signature: string,
    address: string
): Promise<EthSignerType> {
    const messageNoPrefix = getSignedBytesFromMessage(message, false);
    const messageWithPrefix = getSignedBytesFromMessage(message, true);

    const prefixedECDSASigner = utils.recoverAddress(utils.keccak256(messageWithPrefix), signature);
    if (prefixedECDSASigner.toLowerCase() === address.toLowerCase()) {
        return {
            verificationMethod: 'ECDSA',
            isSignedMsgPrefixed: true
        };
    }

    const notPrefixedMsgECDSASigner = utils.recoverAddress(utils.keccak256(messageNoPrefix), signature);
    if (notPrefixedMsgECDSASigner.toLowerCase() === address.toLowerCase()) {
        return {
            verificationMethod: 'ECDSA',
            isSignedMsgPrefixed: false
        };
    }

    let isSignedMsgPrefixed: boolean | null = null;
    // Sometimes an error is thrown if the signature is wrong
    try {
        isSignedMsgPrefixed = await verifyERC1271Signature(address, messageNoPrefix, signature, _provider);
    } catch {
        isSignedMsgPrefixed = false;
    }

    return {
        verificationMethod: 'ERC-1271',
        isSignedMsgPrefixed
    };
}

function removeAddressPrefix(address: Address | PubKeyHash): string {
    if (address.startsWith('0x')) return address.substr(2);

    if (address.startsWith('sync:')) return address.substr(5);

    throw new Error("ETH address must start with '0x' and PubKeyHash must start with 'sync:'");
}

export function serializeContentHash(contentHash: string): Uint8Array {
    const contentHashBytes = utils.arrayify(contentHash);
    if (contentHashBytes.length !== 32) {
        throw new Error('Content hash must be 32 bytes long');
    }

    return contentHashBytes;
}
// PubKeyHash or eth address
export function serializeAddress(address: Address | PubKeyHash): Uint8Array {
    const prefixlessAddress = removeAddressPrefix(address);

    const addressBytes = utils.arrayify(`0x${prefixlessAddress}`);
    if (addressBytes.length !== 20) {
        throw new Error('Address must be 20 bytes long');
    }

    return addressBytes;
}

export function serializeAccountId(accountId: number): Uint8Array {
    if (accountId < 0) {
        throw new Error('Negative account id');
    }
    if (accountId >= MAX_NUMBER_OF_ACCOUNTS) {
        throw new Error('AccountId is too big');
    }
    return numberToBytesBE(accountId, 4);
}

export function serializeTokenId(tokenId: number): Uint8Array {
    if (tokenId < 0) {
        throw new Error('Negative tokenId');
    }
    if (tokenId >= MAX_NUMBER_OF_TOKENS) {
        throw new Error('TokenId is too big');
    }
    return numberToBytesBE(tokenId, 4);
}

export function serializeAmountPacked(amount: BigNumberish): Uint8Array {
    return packAmountChecked(BigNumber.from(amount));
}

export function serializeAmountFull(amount: BigNumberish): Uint8Array {
    const bnAmount = BigNumber.from(amount);
    return utils.zeroPad(utils.arrayify(bnAmount), 16);
}

export function serializeFeePacked(fee: BigNumberish): Uint8Array {
    return packFeeChecked(BigNumber.from(fee));
}

export function serializeNonce(nonce: number): Uint8Array {
    if (nonce < 0) {
        throw new Error('Negative nonce');
    }
    return numberToBytesBE(nonce, 4);
}

export function serializeTimestamp(time: number): Uint8Array {
    if (time < 0) {
        throw new Error('Negative timestamp');
    }
    return ethers.utils.concat([new Uint8Array(4), numberToBytesBE(time, 4)]);
}

export function serializeOrder(order: Order): Uint8Array {
    const type = new Uint8Array(['o'.charCodeAt(0)]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountId = serializeAccountId(order.accountId);
    const recipientBytes = serializeAddress(order.recipient);
    const nonceBytes = serializeNonce(order.nonce);
    const tokenSellId = serializeTokenId(order.tokenSell);
    const tokenBuyId = serializeTokenId(order.tokenBuy);
    const sellPriceBytes = BigNumber.from(order.ratio[0]).toHexString();
    const buyPriceBytes = BigNumber.from(order.ratio[1]).toHexString();
    const amountBytes = serializeAmountPacked(order.amount);
    const validFrom = serializeTimestamp(order.validFrom);
    const validUntil = serializeTimestamp(order.validUntil);
    return ethers.utils.concat([
        type,
        version,
        accountId,
        recipientBytes,
        nonceBytes,
        tokenSellId,
        tokenBuyId,
        ethers.utils.zeroPad(sellPriceBytes, 15),
        ethers.utils.zeroPad(buyPriceBytes, 15),
        amountBytes,
        validFrom,
        validUntil
    ]);
}

export async function serializeSwap(swap: Swap): Promise<Uint8Array> {
    const type = new Uint8Array([255 - 11]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const submitterId = serializeAccountId(swap.submitterId);
    const submitterAddress = serializeAddress(swap.submitterAddress);
    const nonceBytes = serializeNonce(swap.nonce);
    const orderA = serializeOrder(swap.orders[0]);
    const orderB = serializeOrder(swap.orders[1]);
    const ordersHashed = await rescueHashOrders(ethers.utils.concat([orderA, orderB]));
    const tokenIdBytes = serializeTokenId(swap.feeToken);
    const feeBytes = serializeFeePacked(swap.fee);
    const amountABytes = serializeAmountPacked(swap.amounts[0]);
    const amountBBytes = serializeAmountPacked(swap.amounts[1]);
    return ethers.utils.concat([
        type,
        version,
        submitterId,
        submitterAddress,
        nonceBytes,
        ordersHashed,
        tokenIdBytes,
        feeBytes,
        amountABytes,
        amountBBytes
    ]);
}

export function serializeWithdraw(withdraw: Withdraw): Uint8Array {
    const type = new Uint8Array([255 - 3]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountId = serializeAccountId(withdraw.accountId);
    const accountBytes = serializeAddress(withdraw.from);
    const ethAddressBytes = serializeAddress(withdraw.to);
    const tokenIdBytes = serializeTokenId(withdraw.token);
    const amountBytes = serializeAmountFull(withdraw.amount);
    const feeBytes = serializeFeePacked(withdraw.fee);
    const nonceBytes = serializeNonce(withdraw.nonce);
    const validFrom = serializeTimestamp(withdraw.validFrom);
    const validUntil = serializeTimestamp(withdraw.validUntil);
    return ethers.utils.concat([
        type,
        version,
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
}

export function serializeMintNFT(mintNFT: MintNFT): Uint8Array {
    const type = new Uint8Array([255 - 9]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountId = serializeAccountId(mintNFT.creatorId);
    const accountBytes = serializeAddress(mintNFT.creatorAddress);
    const contentHashBytes = serializeContentHash(mintNFT.contentHash);
    const recipientBytes = serializeAddress(mintNFT.recipient);
    const tokenIdBytes = serializeTokenId(mintNFT.feeToken);
    const feeBytes = serializeFeePacked(mintNFT.fee);
    const nonceBytes = serializeNonce(mintNFT.nonce);
    return ethers.utils.concat([
        type,
        version,
        accountId,
        accountBytes,
        contentHashBytes,
        recipientBytes,
        tokenIdBytes,
        feeBytes,
        nonceBytes
    ]);
}

export function serializeWithdrawNFT(withdrawNFT: WithdrawNFT): Uint8Array {
    const type = new Uint8Array([255 - 10]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountId = serializeAccountId(withdrawNFT.accountId);
    const accountBytes = serializeAddress(withdrawNFT.from);
    const ethAddressBytes = serializeAddress(withdrawNFT.to);
    const tokenBytes = serializeTokenId(withdrawNFT.token);
    const tokenIdBytes = serializeTokenId(withdrawNFT.feeToken);
    const feeBytes = serializeFeePacked(withdrawNFT.fee);
    const nonceBytes = serializeNonce(withdrawNFT.nonce);
    const validFrom = serializeTimestamp(withdrawNFT.validFrom);
    const validUntil = serializeTimestamp(withdrawNFT.validUntil);
    return ethers.utils.concat([
        type,
        version,
        accountId,
        accountBytes,
        ethAddressBytes,
        tokenBytes,
        tokenIdBytes,
        feeBytes,
        nonceBytes,
        validFrom,
        validUntil
    ]);
}

export function serializeTransfer(transfer: Transfer): Uint8Array {
    const type = new Uint8Array([255 - 5]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountId = serializeAccountId(transfer.accountId);
    const from = serializeAddress(transfer.from);
    const to = serializeAddress(transfer.to);
    const token = serializeTokenId(transfer.token);
    const amount = serializeAmountPacked(transfer.amount);
    const fee = serializeFeePacked(transfer.fee);
    const nonce = serializeNonce(transfer.nonce);
    const validFrom = serializeTimestamp(transfer.validFrom);
    const validUntil = serializeTimestamp(transfer.validUntil);
    return ethers.utils.concat([type, version, accountId, from, to, token, amount, fee, nonce, validFrom, validUntil]);
}

export function serializeChangePubKey(changePubKey: ChangePubKey): Uint8Array {
    const type = new Uint8Array([255 - 7]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const accountIdBytes = serializeAccountId(changePubKey.accountId);
    const accountBytes = serializeAddress(changePubKey.account);
    const pubKeyHashBytes = serializeAddress(changePubKey.newPkHash);
    const tokenIdBytes = serializeTokenId(changePubKey.feeToken);
    const feeBytes = serializeFeePacked(changePubKey.fee);
    const nonceBytes = serializeNonce(changePubKey.nonce);
    const validFrom = serializeTimestamp(changePubKey.validFrom);
    const validUntil = serializeTimestamp(changePubKey.validUntil);
    return ethers.utils.concat([
        type,
        version,
        accountIdBytes,
        accountBytes,
        pubKeyHashBytes,
        tokenIdBytes,
        feeBytes,
        nonceBytes,
        validFrom,
        validUntil
    ]);
}

export function serializeForcedExit(forcedExit: ForcedExit): Uint8Array {
    const type = new Uint8Array([255 - 8]);
    const version = new Uint8Array([CURRENT_TX_VERSION]);
    const initiatorAccountIdBytes = serializeAccountId(forcedExit.initiatorAccountId);
    const targetBytes = serializeAddress(forcedExit.target);
    const tokenIdBytes = serializeTokenId(forcedExit.token);
    const feeBytes = serializeFeePacked(forcedExit.fee);
    const nonceBytes = serializeNonce(forcedExit.nonce);
    const validFrom = serializeTimestamp(forcedExit.validFrom);
    const validUntil = serializeTimestamp(forcedExit.validUntil);
    return ethers.utils.concat([
        type,
        version,
        initiatorAccountIdBytes,
        targetBytes,
        tokenIdBytes,
        feeBytes,
        nonceBytes,
        validFrom,
        validUntil
    ]);
}

/**
 * Encodes the transaction data as the byte sequence according to the zkSync protocol.
 * @param tx A transaction to serialize.
 */
export function serializeTx(
    tx: Transfer | Withdraw | ChangePubKey | CloseAccount | ForcedExit | MintNFT | WithdrawNFT
): Uint8Array {
    switch (tx.type) {
        case 'Transfer':
            return serializeTransfer(tx);
        case 'Withdraw':
            return serializeWithdraw(tx);
        case 'ChangePubKey':
            return serializeChangePubKey(tx);
        case 'ForcedExit':
            return serializeForcedExit(tx);
        case 'MintNFT':
            return serializeMintNFT(tx);
        case 'WithdrawNFT':
            return serializeWithdrawNFT(tx);
        default:
            return new Uint8Array();
    }
}

export function numberToBytesBE(number: number, bytes: number): Uint8Array {
    const result = new Uint8Array(bytes);
    for (let i = bytes - 1; i >= 0; i--) {
        result[i] = number & 0xff;
        number >>= 8;
    }
    return result;
}

export function parseHexWithPrefix(str: string) {
    return Uint8Array.from(Buffer.from(str.slice(2), 'hex'));
}

export function getCREATE2AddressAndSalt(
    syncPubkeyHash: string,
    create2Data: {
        creatorAddress: string;
        saltArg: string;
        codeHash: string;
    }
): { salt: string; address: string } {
    const pubkeyHashHex = syncPubkeyHash.replace('sync:', '0x');

    const additionalSaltArgument = ethers.utils.arrayify(create2Data.saltArg);
    if (additionalSaltArgument.length !== 32) {
        throw new Error('create2Data.saltArg should be exactly 32 bytes long');
    }

    // CREATE2 salt
    const salt = ethers.utils.keccak256(ethers.utils.concat([additionalSaltArgument, pubkeyHashHex]));

    // Address according to CREATE2 specification
    const address =
        '0x' +
        ethers.utils
            .keccak256(
                ethers.utils.concat([
                    ethers.utils.arrayify(0xff),
                    ethers.utils.arrayify(create2Data.creatorAddress),
                    salt,
                    ethers.utils.arrayify(create2Data.codeHash)
                ])
            )
            .slice(2 + 12 * 2);

    return { address: address, salt: ethers.utils.hexlify(salt) };
}

export async function getEthereumBalance(
    ethProvider: ethers.providers.Provider,
    syncProvider: Provider,
    address: Address,
    token: TokenLike
): Promise<BigNumber> {
    let balance: BigNumber;
    if (isTokenETH(token)) {
        balance = await ethProvider.getBalance(address);
    } else {
        const erc20contract = new Contract(
            syncProvider.tokenSet.resolveTokenAddress(token),
            IERC20_INTERFACE,
            ethProvider
        );

        balance = await erc20contract.balanceOf(address);
    }
    return balance;
}

export async function getPendingBalance(
    ethProvider: ethers.providers.Provider,
    syncProvider: Provider,
    address: Address,
    token: TokenLike
): Promise<BigNumberish> {
    const zksyncContract = new Contract(
        syncProvider.contractAddress.mainContract,
        SYNC_MAIN_CONTRACT_INTERFACE,
        ethProvider
    );

    const tokenAddress = syncProvider.tokenSet.resolveTokenAddress(token);

    return zksyncContract.getPendingBalance(address, tokenAddress);
}

export function getTxHash(tx: Transfer | Withdraw | ChangePubKey | ForcedExit | CloseAccount): string {
    if (tx.type == 'Close') {
        throw new Error('Close operation is disabled');
    }
    let txBytes = serializeTx(tx);
    return ethers.utils.sha256(txBytes).replace('0x', 'sync-tx:');
}
