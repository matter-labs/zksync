/// <reference types="node" />
import { curve } from 'elliptic';
import EdwardsPoint = curve.edwards.EdwardsPoint;
export declare const altjubjubCurve: any;
export declare function pedersenHash(input: Buffer): EdwardsPoint;
