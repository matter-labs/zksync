import Axios from "axios";
import * as qs from "querystring";
import {ethers} from "ethers";
import {gatherSources} from "@resolver-engine/imports";
import {ImportsFsEngine} from "@resolver-engine/imports-fs";

export async function publishSourceCodeToEtherscan(address: string, contractName: string, constructorArguments, contractDirPath = "contracts") {
    const SUPPORTED_NETWORKS = ["mainnet", "rinkeby", "ropsten"];
    const contractPath = `${contractDirPath}/${contractName}.sol`;
    const sourceCode = await getSolidityInput(contractPath);

    const network = process.env.ETH_NETWORK;
    if (SUPPORTED_NETWORKS.find((supportedNetwork) => supportedNetwork === network) == null) {
        throw new Error(`Current network ${network} is not supported by etherscan, should be one of the ${SUPPORTED_NETWORKS.toString()}`);
    }
    const etherscanApiUrl = network === "mainnet" ? "https://api.etherscan.io/api" : `https://api-${network}.etherscan.io/api`;

    const data = {
        action: "verifysourcecode", // Do not change
        apikey: process.env.ETHERSCAN_API_KEY,  // A valid API-Key is required
        codeformat: "solidity-standard-json-input",
        compilerversion: "v0.5.17+commit.d19bba13", // from http://etherscan.io/solcversions
        constructorArguements: constructorArguments, // if applicable. How nice, they have a typo in their api
        contractaddress: address, // Contract Address starts with 0x...
        contractname: `${contractPath}:${contractName}`,
        module: "contract", // Do not change
        sourceCode, // Contract Source Code (Flattened if necessary)
    };

    const r = await Axios.post(etherscanApiUrl, qs.stringify(data));
    const response = r.data;
    if (response.message !== "OK" && response.result !== "Contract source code already verified") {
        throw new Error(`Failed to publish contract code, try again later, ${response}`);
    }
}

export async function publishAbiToTesseracts(address: string, contractCode) {
    const network = process.env.ETH_NETWORK;
    if (network !== "localhost") {
        throw new Error("Only localhost network is supported by Tesseracts");
    }
    const req = {
        contract_source: JSON.stringify(contractCode.abi),
        contract_compiler: "abi-only",
        contract_name: "",
        contract_optimized: false,
    };

    const config = {
        headers: {
            "Content-Type": "application/x-www-form-urlencoded",
        },
    };
    await Axios.post(`http://localhost:8000/${address}/contract`, qs.stringify(req), config);
}

export function encodeConstructorArgs(contractCode, args) {
    const iface = contractCode.abi.filter((i) => i.type === "constructor");
    if (iface.length === 0) {
        return "";
    }
    return ethers.utils.defaultAbiCoder.encode(iface[0].inputs, args).slice(2);
}

export function encodeProxyContstuctorArgs(proxyCode, targetAddress, initArgs, initArgsTypes) {
    const encodedArgs = abiRawEncode(initArgsTypes, initArgs);
    return encodeConstructorArgs(proxyCode, [targetAddress, encodedArgs]);
}

function abiRawEncode(args, vals) {
    return Buffer.from(ethers.utils.defaultAbiCoder.encode(args, vals).slice(2), "hex");
}

async function getSolidityInput(contractPath) {
    let input = await gatherSources([contractPath], process.cwd(), ImportsFsEngine());
    input = input.map((obj) => ({...obj, url: obj.url.replace(`${process.cwd()}/`, "")}));

    const sources: { [s: string]: {} } = {};
    for (const file of input) {
        sources[file.url] = {content: file.source};
    }

    const config = require("../.waffle.json");
    const inputJSON = {
        language: "Solidity",
        settings: {
            outputSelection: {
                "*": {
                    "*": [
                        "abi",
                        "evm.bytecode",
                        "evm.deployedBytecode",
                    ],
                },
            },
            ...config.compilerOptions,
        },
        sources,
    };

    return JSON.stringify(inputJSON, null, 2);
}
