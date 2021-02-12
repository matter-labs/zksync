import { Command } from 'commander';
import { web3Provider } from './utils';
import { BigNumber } from 'ethers';
import * as fs from 'fs';
import { ethers } from 'ethers';
import * as solc from 'solc';

const provider = web3Provider();

const cache: Map<BigNumber, string> = new Map<BigNumber, string>();

async function getStorageAt(address: string, slot: BigNumber): Promise<string> {
    if (!cache.has(slot)) {
        cache.set(slot, (await provider.getStorageAt(address, slot)).substr(2, 64));
    }
    return cache.get(slot);
}

function hexToUtf8(hex: string): string {
    return decodeURIComponent('%' + hex.match(/.{1,2}/g).join('%'));
}

function bytesToHex(array: Uint8Array): string {
    let result: string = '';
    array.forEach((byte) => {
        result += byte.toString(16).padStart(2, '0');
    });
    return result;
}

function utf8ToHex(str: string): string {
    return bytesToHex(ethers.utils.toUtf8Bytes(str));
}

function hexToUnsignedNumber(hex: string): string {
    return BigNumber.from('0x' + hex).toString();
}

function hexToSignedNumber(hex: string): string {
    let bn: bigint = BigInt('0x' + hex);
    if (parseInt(hex.substr(0, 1), 16) > 7) {
        bn =
            BigInt(
                '0b' +
                    bn
                        .toString(2)
                        .split('')
                        .map((i) => {
                            return '0' === i ? 1 : 0;
                        })
                        .join('')
            ) + BigInt(1);
        bn = -bn;
    }
    return bn.toString();
}

function numberTo32Hex(input: string): string {
    const hex = BigNumber.from(input).toHexString();
    return hex.substring(2, hex.length).padStart(64, '0');
}

function hexStringToByte(str) {
    if (!str) {
        return new Uint8Array();
    }
    const a = [];
    for (let i = 0, len = str.length; i < len; i += 2) {
        a.push(parseInt(str.substr(i, 2), 16));
    }
    return new Uint8Array(a);
}

async function readDynamicBytes(slot: BigNumber, address: string): Promise<string> {
    const data = await getStorageAt(address, slot);
    if (Number.parseInt(data.substr(62, 2), 16) % 2 === 0) {
        const length = Number.parseInt(data.substr(62, 2), 16) / 2;
        return data.substr(0, 2 * length);
    } else {
        const length = (Number.parseInt(data, 16) - 1) / 2;
        const firstSlot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
        let hex: string = '';
        for (let slotShift = 0; slotShift * 32 < length; slotShift++) {
            hex += await getStorageAt(address, firstSlot.add(slotShift));
        }
        return hex;
    }
}

async function readBytes(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    const data = await getStorageAt(address, slot);
    return data.substr(64 - bytes * 2 - shift * 2, bytes * 2);
}

async function readString(slot: BigNumber, address: string): Promise<string> {
    return hexToUtf8(await readDynamicBytes(slot, address));
}

async function readSignedNumber(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    return hexToSignedNumber(await readBytes(slot, shift, bytes, address));
}

async function readUnsignedNumber(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    return hexToUnsignedNumber(await readBytes(slot, shift, bytes, address));
}

async function readBoolean(slot: BigNumber, shift: number, address: string): Promise<boolean> {
    return parseInt(await readBytes(slot, shift, 1, address), 16) === 1;
}

async function readAddress(slot: BigNumber, shift: number, address: string): Promise<string> {
    return '0x' + (await readBytes(slot, shift, 20, address));
}

async function readEnum(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    return await readUnsignedNumber(slot, shift, bytes, address);
}

let types: any;

async function readPrimitive(slot: BigNumber, shift: number, address: string, type: string): Promise<any> {
    if (type.substr(0, 5) === 't_int') return readSignedNumber(slot, shift, types[type].numberOfBytes, address);
    if (type.substr(0, 6) === 't_uint') return readUnsignedNumber(slot, shift, types[type].numberOfBytes, address);
    if (type === 't_bool') return readBoolean(slot, shift, address);
    if (type === 't_address' || type === 't_address_payable') return readAddress(slot, shift, address);
    if (type === 't_bytes_storage') return readDynamicBytes(slot, address);
    if (type.substr(0, 7) === 't_bytes') return readBytes(slot, shift, types[type].numberOfBytes, address);
    if (type === 't_string_storage') return readString(slot, address);
    if (type.substr(0, 6) === 't_enum') return readEnum(slot, shift, types[type].numberOfBytes, address);
    return undefined;
}

async function readStruct(slot: BigNumber, address: string, type: string): Promise<object> {
    const result = {};
    for (const member of types[type].members) {
        result[member.label] = await readVariable(
            slot.add(Number.parseInt(member.slot, 10)),
            member.offset,
            address,
            member.type
        );
    }
    return result;
}

async function readArray(slot: BigNumber, address: string, type: string): Promise<any> {
    const result = [];
    let length: number;
    const baseType = types[type].base;
    if (types[type].encoding === 'dynamic_array') {
        length = Number.parseInt(await getStorageAt(address, slot), 16);
        slot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
    } else {
        length = Number.parseInt(type.substring(type.lastIndexOf(')') + 1, type.lastIndexOf('_')), 10);
    }
    if (Number.parseInt(types[baseType].numberOfBytes, 10) < 32) {
        let shift: number = -Number.parseInt(types[baseType].numberOfBytes, 10);
        for (let i = 0; i < length; i++) {
            shift += Number.parseInt(types[baseType].numberOfBytes, 10);
            if (shift + Number.parseInt(types[baseType].numberOfBytes, 10) > 32) {
                shift = 0;
                slot = slot.add(1);
            }
            result.push(await readVariable(slot, shift, address, baseType));
        }
    } else {
        for (let i = 0; i < length; i++) {
            result.push(await readVariable(slot, 0, address, baseType));
            slot = slot.add(Number.parseInt(types[baseType].numberOfBytes, 10) / 32);
        }
    }
    return result;
}

async function readVariable(slot: BigNumber, shift: number, address: string, type: string): Promise<any> {
    if (type.substr(0, 7) === 't_array') return readArray(slot, address, type);
    if (type.substr(0, 8) === 't_struct') return readStruct(slot, address, type);
    return readPrimitive(slot, shift, address, type);
}

async function readPartOfStruct(slot: BigNumber, address: string, type: string, params: string[]): Promise<object> {
    const last = params.pop();
    for (const member of types[type].members) {
        if (member.label === last) {
            return readPartOfVariable(
                slot.add(Number.parseInt(member.slot, 10)),
                member.offset,
                address,
                member.type,
                params
            );
        }
    }
}

async function readPartOfArray(slot: BigNumber, address: string, type: string, params: string[]): Promise<any> {
    const index = Number.parseInt(params.pop(), 10);
    const baseType = types[type].base;
    if (types[type].encoding === 'dynamic_array') {
        slot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
    }
    if (Number.parseInt(types[baseType].numberOfBytes, 10) < 32) {
        const inOne = Math.floor(32 / Number.parseInt(types[baseType].numberOfBytes, 10));
        const shift = Number.parseInt(types[baseType].numberOfBytes, 10) * (index % inOne);
        return readPartOfVariable(slot.add(Math.floor(index / inOne)), shift, address, baseType, params);
    } else {
        return readPartOfVariable(
            slot.add((index * Number.parseInt(types[baseType].numberOfBytes, 10)) / 32),
            0,
            address,
            baseType,
            params
        );
    }
}

function parseKey(key: string, type: string): string {
    if (type === 't_bool') {
        if (key === 'false' || key === '0') return numberTo32Hex('0');
        else return numberTo32Hex('1');
    }
    if (type === 't_address' || type === 't_address_payable') {
        if (key.length === 42) return key.substring(2, key.length).padStart(64, '0');
        else return key.padStart(64, ' 0');
    }
    if (type === 't_string_memory_ptr') {
        return utf8ToHex(key);
    }
    if (type === 't_bytes_memory_ptr') {
        if (key.substr(0, 2) === '0x') return key.substring(2, key.length);
        else return key;
    }
    return numberTo32Hex(key);
}

async function readPartOfMap(slot: BigNumber, address: string, type: string, params: string[]): Promise<any> {
    const key = params.pop();
    const valueType = types[type].value;
    const keyType = types[type].key;
    slot = BigNumber.from(
        ethers.utils.keccak256(hexStringToByte(parseKey(key, keyType) + numberTo32Hex(slot.toString())))
    );
    return readPartOfVariable(slot, 0, address, valueType, params);
}

async function readPartOfVariable(
    slot: BigNumber,
    shift: number,
    address: string,
    type: string,
    params: string[]
): Promise<any> {
    if (params.length === 0) {
        return readVariable(slot, shift, address, type);
    }
    if (type.substr(0, 7) === 't_array') {
        return readPartOfArray(slot, address, type, params);
    }
    if (type.substr(0, 8) === 't_struct') {
        return readPartOfStruct(slot, address, type, params);
    }
    if (types[type].encoding === 'mapping') {
        return readPartOfMap(slot, address, type, params);
    }
    return readPrimitive(slot, shift, address, type);
}

function parseName(fullName: string): string[] {
    const firstPoint = fullName.indexOf('.');
    const firstBracket = fullName.indexOf('[');
    if (firstPoint === -1 && firstBracket === -1) {
        return [];
    }
    const result = [];
    let first: number;
    if (firstPoint === -1) {
        first = firstBracket;
    } else if (firstBracket === -1) {
        first = firstPoint;
    } else {
        first = Math.min(firstPoint, firstBracket);
    }
    let str: string = '';
    let bracket: boolean = false;
    for (let i = first; i < fullName.length; i++) {
        if (fullName.charAt(i) === '.') {
            if (bracket) {
                throw new Error('Invalid name');
            }
            if (i !== first) result.push(str);
            str = '';
        } else if (fullName.charAt(i) === '[') {
            if (bracket) {
                throw new Error('Invalid name');
            }
            bracket = true;
            if (i !== first) result.push(str);
            str = '';
        } else if (fullName.charAt(i) === ']') {
            if (!bracket) {
                throw new Error('Invalid name');
            }
            bracket = false;
        } else {
            str += fullName.charAt(i);
        }
    }
    if (bracket) {
        throw new Error('Invalid name');
    }
    result.push(str);
    return result.reverse();
}

function getVariableName(fullName: string): string {
    let variableName: string = fullName;
    if (variableName.indexOf('[') !== -1) {
        variableName = variableName.substr(0, variableName.indexOf('['));
    }
    if (variableName.indexOf('.') !== -1) {
        variableName = variableName.substr(0, variableName.indexOf('.'));
    }
    return variableName;
}

function compileAndGetStorage(file: string, contractName): any {
    const contractCode = fs.readFileSync(file, {
        encoding: 'utf-8'
    });
    const sourceFiles = {};
    sourceFiles[`${contractName}.sol`] = { content: contractCode };
    const input = {
        language: 'Solidity',
        sources: sourceFiles,
        settings: {
            outputSelection: {
                '*': {
                    '*': ['storageLayout']
                }
            }
        }
    };
    const output = JSON.parse(solc.compile(JSON.stringify(input)));
    if (output.contracts === undefined) {
        throw new Error(JSON.stringify(output.errors));
    }
    types = output.contracts[`${contractName}.sol`][contractName].storageLayout.types;
    return output.contracts[`${contractName}.sol`][contractName].storageLayout.storage;
}

async function getValue(address: string, contractName: string, name: string, file?: string): Promise<any> {
    if (file === undefined) {
        file = `${process.env.ZKSYNC_HOME}/contracts/contracts/${contractName}.sol`;
    }
    const storage = compileAndGetStorage(file, contractName);

    const variableName = getVariableName(name);
    const params = parseName(name);
    let variable: any;

    storage.forEach((node) => {
        if (node.label === variableName) variable = node;
    });
    if (variable === undefined) {
        throw new Error('Invalid name');
    }
    return readPartOfVariable(BigNumber.from(variable.slot), variable.offset, address, variable.type, params);
}

async function main() {
    const program = new Command();

    program.version('0.1.0').name('read-variable').description('returns value of private and public variables');

    program
        .command('read <address> <contractName> <variableName>')
        .option('-f, --file <file>')
        .description('Reads value of variable')
        .action(async (address: string, contractName: string, variableName: string, cmd: Command) => {
            console.log(JSON.stringify(await getValue(address, contractName, variableName, cmd.file), null, 4));
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
