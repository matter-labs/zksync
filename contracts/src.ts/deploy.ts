import { deployContract } from 'ethereum-waffle';
import { ethers } from 'ethers';
import { bigNumberify, parseEther } from "ethers/utils";
import Axios from "axios";
import * as qs from 'querystring';
import * as url from 'url';
import * as fs from 'fs';
import * as path from 'path';

const abi = require('ethereumjs-abi')
const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = { bytecode: contract.bytecode };
    return contract
}();

export const proxyContractCode = require(`../flat_build/Proxy`);
export const franklinContractCode = require(`../flat_build/Franklin`);
export const verifierContractCode = require(`../flat_build/Verifier`);
export const governanceContractCode = require(`../flat_build/Governance`);

export const proxyContractSourceCode = fs.readFileSync('flat/Proxy.sol', 'utf8');
export const franklinContractSourceCode = fs.readFileSync('flat/Franklin.sol', 'utf8');
export const verifierContractSourceCode = fs.readFileSync('flat/Verifier.sol', 'utf8');
export const governanceContractSourceCode = fs.readFileSync('flat/Governance.sol', 'utf8');

export const proxyTestContractCode = require('../build/ProxyTest');
export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');

export async function publishSourceCodeToEtherscan(contractname, contractaddress, sourceCode, compiled, constructorParams: any[]) {
    const network = process.env.ETH_NETWORK;
    const etherscanApiUrl = network === 'mainnet' ? 'https://api.etherscan.io/api' : `https://api-${network}.etherscan.io/api`;

    let constructorArguments;
    if (constructorParams) {
        let constructorInputs = compiled
            .abi
            .filter(i => i.type === 'constructor');

        if (constructorInputs.length > 0) {
            constructorArguments =
                ethers.utils.defaultAbiCoder
                    .encode(
                        constructorInputs[0].inputs,
                        constructorParams
                    )
                    .slice(2);
        }
    }

    let data = {
        apikey: process.env.ETHERSCAN_API_KEY,  // A valid API-Key is required
        module: 'contract',                     // Do not change
        action: 'verifysourcecode',             // Do not change
        contractaddress,                                    // Contract Address starts with 0x...     
        sourceCode,                                         // Contract Source Code (Flattened if necessary)
        contractname,                                       // ContractName
        compilerversion: 'v0.5.16+commit.9c3226ce',      // see http://etherscan.io/solcversions for list of support versions
        optimizationUsed: 0,                              // 0 = No Optimization, 1 = Optimization used
        runs: 200,                            // set to 200 as default unless otherwise
        constructorArguements: constructorArguments         // if applicable. How nice, they have a typo in their api
    };

    let r = await Axios.post(etherscanApiUrl, qs.stringify(data));
    let retriesLeft = 20;
    if (r.data.status != 1) {
        if (r.data.result.includes('Unable to locate ContractCode')) {
            // waiting for etherscan backend and try again
            await sleep(15000);
            if (retriesLeft > 0) {
                --retriesLeft;
                await publishSourceCodeToEtherscan(contractname, contractaddress, sourceCode, compiled, constructorParams);
            }
        } else {
            console.error(`Problem publishing ${contractname}:`, r.data);
        }
    } else {
        let status;
        let retriesLeft = 10;
        while (retriesLeft-- > 0) {
            status = await Axios.get(`http://api.etherscan.io/api?module=contract&&action=checkverifystatus&&guid=${r.data.result}`).then(r => r.data);

            if (status.result.includes('Pending in queue') == false)
                break;

            await sleep(5000);
        }

        console.log(`Published ${contractname} sources on https://${network}.etherscan.io/address/${contractaddress} with status`, status);
    }
}

export async function deployProxy(
    wallet,
    proxyCode,
) {
    try {
        const proxy = await deployContract(wallet, proxyCode, [], {
            gasLimit: 3000000,
        });

        return proxy;
    } catch (err) {
        console.error("Proxy deploy error:" + err);
    }
}

export async function deployGovernance(
    wallet,
    proxyCode,
    governanceCode,
    initArgs,
    initArgsValues,
) {
    try {
        const proxy = await deployProxy(wallet, proxyCode);
        const governance = await deployContract(wallet, governanceCode, [], {
            gasLimit: 3000000,
        });
        const initArgsInBytes = await abi.rawEncode(initArgs, initArgsValues);
        const tx = await proxy.initialize(governance.address, initArgsInBytes);
        await tx.wait();

        const returnContract = new ethers.Contract(proxy.address, governanceCode.interface, wallet);
        return [returnContract, governance.address];
    } catch (err) {
        console.log("Governance deploy error:" + err);
    }
}

export async function deployVerifier(
    wallet,
    proxyCode,
    verifierCode,
    initArgs,
    initArgsValues,
) {
    try {
        const proxy = await deployProxy(wallet, proxyCode);
        const verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 3000000,
        });
        const initArgsInBytes = await abi.rawEncode(initArgs, initArgsValues);
        const tx = await proxy.initialize(verifier.address, initArgsInBytes);
        await tx.wait();

        const returnContract = new ethers.Contract(proxy.address, verifierCode.interface, wallet);
        return [returnContract, verifier.address];
    } catch (err) {
        console.error("Verifier deploy error:" + err);
    }
}

export async function deployFranklin(
    wallet,
    proxyCode,
    franklinCode,
    initArgs,
    initArgsValues,
) {
    try {
        let [
            governanceProxyAddress,
            verifierProxyAddress,
            genesisAddress,
            genesisRoot
        ] = initArgsValues;

        const proxy = await deployProxy(wallet, proxyCode);
        const contract = await deployContract(
            wallet,
            franklinCode,
            [],
            {
                gasLimit: 6000000,
            });
        const initArgsInBytes = await abi.rawEncode(initArgs, initArgsValues);
        const initTx = await proxy.initialize(contract.address, initArgsInBytes);
        await initTx.wait();

        const returnContract = new ethers.Contract(proxy.address, franklinCode.interface, wallet);
        return [returnContract, contract.address];
    } catch (err) {
        console.log("Franklin deploy error:" + err);
    }
}

export async function postContractToTesseracts(contractCode, contractName: string, address: string) {
    let req = {
        contract_source: JSON.stringify(contractCode.abi),
        contract_compiler: "abi-only",
        contract_name: contractName,
        contract_optimized: false
    };

    const config = {
        headers: {
            'Content-Type': 'application/x-www-form-urlencoded'
        }
    };
    await Axios.post(`http://localhost:8000/${address}/contract`, qs.stringify(req), config);
}

export async function addTestERC20Token(wallet, governance) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, parseEther("3000000000"));
        await (await governance.addToken(erc20.address)).wait();
        return erc20;
    } catch (err) {
        console.error("Add token error:" + err);
    }
}

export async function mintTestERC20Token(wallet, erc20) {
    try {
        const txCall = await erc20.mint(wallet.address, parseEther("3000000000"));
        await txCall.wait();
    } catch (err) {
        console.error("Mint token error:" + err);
    }
}

export async function addTestNotApprovedERC20Token(wallet) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        return erc20;
    } catch (err) {
        console.error("Add token error:" + err);
    }
}
