/// <reference types="node" />
import BN = require('bn.js');
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
import edwards = curve.edwards;
export declare const altjubjubCurve: any;
export declare const addressLen = 20;
export declare function pedersenHash(input: Buffer, bit_endianness?: "le" | "be"): EdwardsPoint;
export declare function musigSHA256(priv_key: BN, msg: Buffer): {
    pub_key: string;
    sign: string;
};
export declare function musigPedersen(priv_key: BN, msg: Buffer): {
    pub_key: string;
    sign: string;
};
export declare function privateKeyToPublicKey(pk: BN): edwards.EdwardsPoint;
export declare function pubkeyToAddress(pubKey: edwards.EdwardsPoint): Buffer;
export declare function serializePointPacked(point: edwards.EdwardsPoint): Buffer;
export declare function signTransactionBytes(privKey: BN, bytes: Buffer): {
    pub_key: string;
    sign: string;
};
export declare function privateKeyFromSeed(seed: Buffer): {
    privateKey: BN;
    publicKey: EdwardsPoint;
};
