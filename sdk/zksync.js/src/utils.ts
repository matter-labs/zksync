import { utils, constants, ethers, BigNumber, BigNumberish } from 'ethers';
import { PubKeyHash, TokenAddress, TokenLike, Tokens, TokenSymbol, EthSignerType, Address } from './types';

// Max number of tokens for the current version, it is determined by the zkSync circuit implementation.
const MAX_NUMBER_OF_TOKENS = 128;
// Max number of accounts for the current version, it is determined by the zkSync circuit implementation.
const MAX_NUMBER_OF_ACCOUNTS = Math.pow(2, 24);

export const IERC20_INTERFACE = new utils.Interface(require('../abi/IERC20.json').abi);
export const SYNC_MAIN_CONTRACT_INTERFACE = new utils.Interface(require('../abi/SyncMain.json').abi);

export const SYNC_GOV_CONTRACT_INTERFACE = new utils.Interface(require('../abi/SyncGov.json').abi);

export const IEIP1271_INTERFACE = new utils.Interface(require('../abi/IEIP1271.json').abi);

export const MAX_ERC20_APPROVE_AMOUNT =
    '115792089237316195423570985008687907853269984665640564039457584007913129639935'; // 2^256 - 1

export const ERC20_APPROVE_TRESHOLD = '57896044618658097711785492504343953926634992332820282019728792003956564819968'; // 2^255

export const ERC20_DEPOSIT_GAS_LIMIT = BigNumber.from('300000'); // 300k

const AMOUNT_EXPONENT_BIT_WIDTH = 5;
const AMOUNT_MANTISSA_BIT_WIDTH = 35;
const FEE_EXPONENT_BIT_WIDTH = 5;
const FEE_MANTISSA_BIT_WIDTH = 11;

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

export function integerToFloat(
    integer: BigNumber,
    exp_bits: number,
    mantissa_bits: number,
    exp_base: number
): Uint8Array {
    const max_exponent = BigNumber.from(10).pow(Math.pow(2, exp_bits) - 1);
    const max_mantissa = BigNumber.from(2).pow(mantissa_bits).sub(1);

    if (integer.gt(max_mantissa.mul(max_exponent))) {
        throw new Error('Integer is too big');
    }

    let exponent = 0;
    let mantissa = integer;
    while (mantissa.gt(max_mantissa)) {
        mantissa = mantissa.div(exp_base);
        exponent += 1;
    }

    // encode into bits. First bits of mantissa in LE order
    const encoding = [];

    encoding.push(...numberToBits(exponent, exp_bits));
    const mantissaNumber = mantissa.toNumber();
    encoding.push(...numberToBits(mantissaNumber, mantissa_bits));

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

function packFee(amount: BigNumber): Uint8Array {
    return reverseBits(integerToFloat(amount, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, 10));
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

export function isTransactionFeePackable(amount: BigNumberish): boolean {
    return closestPackableTransactionFee(amount).eq(amount);
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

export function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export function isTokenETH(token: TokenLike): boolean {
    return token === 'ETH' || token === constants.AddressZero;
}

export class TokenSet {
    // TODO: Replace with hardcoded list of tokens for final version this is temporary solution
    //  so that we can get list of the supported from zksync node,
    constructor(private tokensBySymbol: Tokens) {}

    private resolveTokenObject(tokenLike: TokenLike) {
        if (this.tokensBySymbol[tokenLike]) {
            return this.tokensBySymbol[tokenLike];
        }

        for (const token of Object.values(this.tokensBySymbol)) {
            if (token.address.toLocaleLowerCase() == tokenLike.toLocaleLowerCase()) {
                return token;
            }
        }
        throw new Error(`Token ${tokenLike} is not supported`);
    }

    public isTokenTransferAmountPackable(tokenLike: TokenLike, amount: string): boolean {
        const parsedAmount = this.parseToken(tokenLike, amount);
        return isTransactionAmountPackable(parsedAmount);
    }

    public isTokenTransactionFeePackable(tokenLike: TokenLike, amount: string): boolean {
        const parsedAmount = this.parseToken(tokenLike, amount);
        return isTransactionFeePackable(parsedAmount);
    }

    public formatToken(tokenLike: TokenLike, amount: BigNumberish): string {
        const decimals = this.resolveTokenDecimals(tokenLike);
        return utils.formatUnits(amount, decimals);
    }

    public parseToken(tokenLike: TokenLike, amount: string): BigNumber {
        const decimals = this.resolveTokenDecimals(tokenLike);
        return utils.parseUnits(amount, decimals);
    }

    public resolveTokenDecimals(tokenLike: TokenLike): number {
        return this.resolveTokenObject(tokenLike).decimals;
    }

    public resolveTokenId(tokenLike: TokenLike): number {
        return this.resolveTokenObject(tokenLike).id;
    }

    public resolveTokenAddress(tokenLike: TokenLike): TokenAddress {
        return this.resolveTokenObject(tokenLike).address;
    }

    public resolveTokenSymbol(tokenLike: TokenLike): TokenSymbol {
        return this.resolveTokenObject(tokenLike).symbol;
    }
}

export function getChangePubkeyMessage(pubKeyHash: PubKeyHash, nonce: number, accountId: number): string {
    const msgNonce = utils.hexlify(serializeNonce(nonce));
    const msgAccId = utils.hexlify(serializeAccountId(accountId));
    const pubKeyHashHex = pubKeyHash.replace('sync:', '').toLowerCase();
    const message =
        `Register zkSync pubkey:\n\n` +
        `${pubKeyHashHex}\n` +
        `nonce: ${msgNonce}\n` +
        `account id: ${msgAccId}\n\n` +
        `Only sign this message for a trusted client!`;
    return message;
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
    const hash = utils.keccak256(message);
    const eip1271 = new ethers.Contract(address, IEIP1271_INTERFACE, signerOrProvider);
    const eipRetVal = await eip1271.isValidSignature(utils.hexlify(hash), signature);
    return eipRetVal === EIP1271_SUCCESS_VALUE;
}

export async function getEthSignatureType(
    provider: ethers.providers.Provider,
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

    return {
        verificationMethod: 'ERC-1271',
        isSignedMsgPrefixed: true
    };
}

function removeAddressPrefix(address: Address | PubKeyHash): string {
    if (address.startsWith('0x')) return address.substr(2);

    if (address.startsWith('sync:')) return address.substr(5);

    throw new Error("ETH address must start with '0x' and PubKeyHash must start with 'sync:'");
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
    return numberToBytesBE(tokenId, 2);
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

function numberToBytesBE(number: number, bytes: number): Uint8Array {
    const result = new Uint8Array(bytes);
    for (let i = bytes - 1; i >= 0; i--) {
        result[i] = number & 0xff;
        number >>= 8;
    }
    return result;
}
