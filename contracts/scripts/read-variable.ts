import { Command } from 'commander';
import { web3Provider } from './utils';
import { BigNumber } from 'ethers';
import { ethers } from 'ethers';
import * as hre from 'hardhat';

const provider = web3Provider();

const cache: Map<BigNumber, string> = new Map<BigNumber, string>();

async function getStorageAt(address: string, slot: BigNumber): Promise<string> {
    if (!cache.has(slot)) {
        cache.set(slot, await provider.getStorageAt(address, slot));
    }
    return cache.get(slot);
}

// Read bytes from storage like hex string
async function readBytes(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    const data = await getStorageAt(address, slot);
    return '0x' + data.substr(66 - bytes * 2 - shift * 2, bytes * 2);
}

// Read dynamic sized bytes (encoding: bytes)
async function readDynamicBytes(slot: BigNumber, address: string): Promise<string> {
    const data = await getStorageAt(address, slot);
    if (Number.parseInt(data.substr(64, 2), 16) % 2 === 0) {
        const length = Number.parseInt(data.substr(64, 2), 16) / 2;
        return '0x' + data.substr(2, 2 * length);
    } else {
        const length = (Number.parseInt(data, 16) - 1) / 2;
        const firstSlot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
        const slots = [];
        for (let slotShift = 0; slotShift * 32 < length; slotShift++) {
            slots.push(getStorageAt(address, firstSlot.add(slotShift)));
        }

        const lastLength = length % 32;
        let hex: string = '0x';
        for (let i = 0; i < slots.length; i++) {
            if (i === slots.length - 1) {
                hex += (await slots[i]).substr(2, lastLength * 2);
            } else {
                hex += (await slots[i]).substr(2, 64);
            }
        }
        return hex;
    }
}

// Functions for read all types, except user defined structs and arrays
async function readString(slot: BigNumber, address: string): Promise<string> {
    return ethers.utils.toUtf8String(await readDynamicBytes(slot, address));
}

async function readNumber(slot: BigNumber, shift: number, label: string, address: string): Promise<string> {
    let bytes: number;
    if (label.substr(0, 3) === 'int') {
        bytes = +label.substring(3, label.length) / 8;
    } else {
        bytes = +label.substring(4, label.length) / 8;
    }
    let data: string = await readBytes(slot, shift, bytes, address);
    data = ethers.utils.hexZeroPad(data, 32);
    return ethers.utils.defaultAbiCoder.decode([label], data).toString();
}

async function readBoolean(slot: BigNumber, shift: number, address: string): Promise<boolean> {
    return (await readNumber(slot, shift, 'uint8', address)) !== '0';
}

async function readAddress(slot: BigNumber, shift: number, address: string): Promise<string> {
    return readBytes(slot, shift, 20, address);
}

async function readEnum(slot: BigNumber, shift: number, bytes: number, address: string): Promise<string> {
    return await readNumber(slot, shift, 'uint' + bytes * 8, address);
}

let types: any;

async function readPrimitive(slot: BigNumber, shift: number, address: string, type: string): Promise<any> {
    if (type.substr(0, 5) === 't_int' || type.substr(0, 6) === 't_uint') {
        return readNumber(slot, shift, types[type].label, address);
    }
    if (type === 't_bool') {
        return readBoolean(slot, shift, address);
    }
    if (type === 't_address' || type === 't_address_payable') {
        return readAddress(slot, shift, address);
    }
    if (type === 't_bytes_storage') {
        return readDynamicBytes(slot, address);
    }
    if (type.substr(0, 7) === 't_bytes') {
        return readBytes(slot, shift, types[type].numberOfBytes, address);
    }
    if (type === 't_string_storage') {
        return readString(slot, address);
    }
    if (type.substr(0, 6) === 't_enum') {
        return readEnum(slot, shift, types[type].numberOfBytes, address);
    }
}

// Read user defined struct
async function readStruct(slot: BigNumber, address: string, type: string): Promise<object> {
    const result = {};
    const data = new Map();
    types[type].members.forEach((member) => {
        data.set(
            member.label,
            readVariable(slot.add(Number.parseInt(member.slot, 10)), member.offset, address, member.type)
        );
    });
    for (const [key, value] of data) {
        result[key] = await value;
    }
    return result;
}

// Read array (Static or dynamic sized)
async function readArray(slot: BigNumber, address: string, type: string): Promise<any[]> {
    let length: number;
    const baseType = types[type].base;
    if (types[type].encoding === 'dynamic_array') {
        length = +(await readNumber(slot, 0, 'uint256', address));
        slot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
    } else {
        length = Number.parseInt(type.substring(type.lastIndexOf(')') + 1, type.lastIndexOf('_')), 10);
    }
    const baseBytes = +types[baseType].numberOfBytes;
    const data = [];
    if (baseBytes < 32) {
        let shift: number = -baseBytes;
        for (let i = 0; i < length; i++) {
            shift += baseBytes;
            if (shift + baseBytes > 32) {
                shift = 0;
                slot = slot.add(1);
            }
            data.push(readVariable(slot, shift, address, baseType));
        }
    } else {
        for (let i = 0; i < length; i++) {
            data.push(readVariable(slot, 0, address, baseType));
            slot = slot.add(baseBytes / 32);
        }
    }
    return Promise.all(data);
}

// Read any type, except mapping (it needs key for reading)
async function readVariable(slot: BigNumber, shift: number, address: string, type: string): Promise<any> {
    if (type.substr(0, 7) === 't_array') return readArray(slot, address, type);
    if (type.substr(0, 8) === 't_struct') return readStruct(slot, address, type);
    return readPrimitive(slot, shift, address, type);
}

// Read field of struct
async function readPartOfStruct(slot: BigNumber, address: string, type: string, params: string[]): Promise<any> {
    const last = params.pop();
    const member = types[type].members.find((element) => {
        return element.label === last;
    });
    if (!member) throw new Error('Invalid field name of struct');
    return readPartOfVariable(slot.add(Number.parseInt(member.slot, 10)), member.offset, address, member.type, params);
}

// Read array element by index
async function readPartOfArray(slot: BigNumber, address: string, type: string, params: string[]): Promise<any> {
    const index = +params.pop();
    const baseType = types[type].base;
    if (types[type].encoding === 'dynamic_array') {
        slot = BigNumber.from(ethers.utils.solidityKeccak256(['uint'], [slot]));
    }
    const baseBytes = +types[baseType].numberOfBytes;
    if (baseBytes < 32) {
        const inOne = Math.floor(32 / baseBytes);
        slot = slot.add(Math.floor(index / inOne));
        const shift = baseBytes * (index % inOne);
        return readPartOfVariable(slot, shift, address, baseType, params);
    } else {
        slot = slot.add((index * baseBytes) / 32);
        return readPartOfVariable(slot, 0, address, baseType, params);
    }
}

// Encode key for mapping type
function encodeKey(key: string, type: string): string {
    if (type === 't_bool') {
        if (key === 'false' || key === '0') {
            return ethers.utils.defaultAbiCoder.encode(['bool'], [false]);
        } else {
            return ethers.utils.defaultAbiCoder.encode(['bool'], [true]);
        }
    }
    if (type === 't_address' || type === 't_address_payable') {
        if (key.length === 42) {
            return '0x' + key.substring(2, key.length).padStart(64, '0');
        } else {
            return '0x' + key.padStart(64, '0');
        }
    }
    if (type === 't_string_memory_ptr') {
        return ethers.utils.hexlify(ethers.utils.toUtf8Bytes(key));
    }
    if (type === 't_bytes_memory_ptr') {
        if (key.substr(0, 2) === '0x') {
            return key;
        } else {
            return '0x' + key;
        }
    }
    return ethers.utils.defaultAbiCoder.encode([types[type].label], [+key]);
}

// Read mapping element by key
async function readPartOfMap(slot: BigNumber, address: string, type: string, params: string[]): Promise<any> {
    const key = params.pop();
    const valueType = types[type].value;
    const keyType = types[type].key;
    const encodedKey = encodeKey(key, keyType);
    const encodedSlot = ethers.utils.defaultAbiCoder.encode(['uint'], [slot]);
    const hex = encodedKey + encodedSlot.substring(2, encodedSlot.length);
    slot = BigNumber.from(ethers.utils.keccak256(ethers.utils.arrayify(hex)));
    return readPartOfVariable(slot, 0, address, valueType, params);
}

// Read part of variable (By indexes, field names, keys)
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

// Get reverse array of indexes, struct fields and mapping keys from name
// Field names, keys cannot contain square brackets or points
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
                throw new Error('Field names, keys cannot contain square brackets or points');
            }
            if (i !== first) result.push(str);
            str = '';
        } else if (fullName.charAt(i) === '[') {
            if (bracket) {
                throw new Error('Field names, keys cannot contain square brackets or points');
            }
            bracket = true;
            if (i !== first) result.push(str);
            str = '';
        } else if (fullName.charAt(i) === ']') {
            if (!bracket) {
                throw new Error('Field names, keys cannot contain square brackets or points');
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

// Get variableName (Without indexes, fields, keys)
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

async function getValue(address: string, contractName: string, name: string): Promise<any> {
    // Mapping from Contract name to fully qualified name known to hardhat
    // e.g ZkSync => cache/solpp-generated-contracts/ZkSync.sol
    const contractMap = {};
    const allContracts = await hre.artifacts.getAllFullyQualifiedNames();
    allContracts.forEach((fullName) => {
        const [file, contract] = fullName.split(':');
        contractMap[contract] = {
            fullName,
            file
        };
    });
    if (!(contractName in contractMap)) {
        throw new Error(`Unknown contract name, available contracts: ${Object.keys(contractMap)}`);
    }
    const buildInfo = await hre.artifacts.getBuildInfo(contractMap[contractName].fullName);
    // @ts-ignore
    const layout = buildInfo.output.contracts[contractMap[contractName].file][contractName].storageLayout;
    types = layout.types;
    const storage = layout.storage;

    const variableName = getVariableName(name);
    const params = parseName(name);
    let variable: any;

    storage.forEach((node) => {
        if (node.label === variableName) variable = node;
    });
    if (!variable) {
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
        .action(async (address: string, contractName: string, variableName: string) => {
            console.log(JSON.stringify(await getValue(address, contractName, variableName), null, 4));
        });

    await program.parseAsync(process.argv);
}

main()
    .then(() => process.exit(0))
    .catch((err) => {
        console.error('Error:', err.message || err);
        process.exit(1);
    });
