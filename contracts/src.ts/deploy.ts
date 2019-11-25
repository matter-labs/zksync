import {deployContract} from 'ethereum-waffle';
import {ethers} from 'ethers';
import {bigNumberify} from "ethers/utils";
import Axios from "axios";
import * as qs from 'querystring';
import * as url from 'url';
import * as fs from 'fs';
import * as path from 'path';


const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();

export const franklinContractCode = require('../flat_build/Franklin');
export const verifierContractCode = require('../flat_build/Verifier');
export const governanceContractCode = require('../flat_build/Governance');
export const priorityQueueContractCode = require('../flat_build/PriorityQueue')

export const franklinContractSourceCode = fs.readFileSync('flat/Franklin.sol', 'utf8');
export const verifierContractSourceCode = fs.readFileSync('flat/Verifier.sol', 'utf8');
export const governanceContractSourceCode = fs.readFileSync('flat/Governance.sol', 'utf8');
export const priorityQueueContractSourceCode = fs.readFileSync('flat/PriorityQueue.sol', 'utf8');

export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');
export const priorityQueueTestContractCode = require('../build/PriorityQueueTest')

export async function publishSourceCode(contractname, contractaddress, sourceCode, compiled, constructorParams: any[]) {
    const network = process.env.ETH_NETWORK;
    const etherscanApiUrl = network === 'mainnet' ? 'https://api.etherscan.io/api' : `https://api-${network}.etherscan.io/api`;

    let constructorArguments;
    if (constructorParams) {
        let constructorInputs = compiled
            .abi
            .filter(i => i.type === 'constructor');
            
        if (constructorInputs.length > 0) {
            constructorArguments = ethers.utils.defaultAbiCoder.encode(
                constructorInputs[0].inputs,
                constructorParams
            );
        }
    }

    let data = {
        apikey:             process.env.ETHERSCAN_API_KEY,  // A valid API-Key is required        
        module:             'contract',                     // Do not change
        action:             'verifysourcecode',             // Do not change
        contractaddress,                                    // Contract Address starts with 0x...     
        sourceCode,                                         // Contract Source Code (Flattened if necessary)
        contractname,                                       // ContractName
        compilerversion:    'v0.5.10+commit.5a6ea5b1',      // see http://etherscan.io/solcversions for list of support versions
        optimizationUsed:   0,                              // 0 = No Optimization, 1 = Optimization used
        runs:               200,                            // set to 200 as default unless otherwise         
        constructorArguments                                // if applicable
    };
    
    let r = await Axios.post(etherscanApiUrl, qs.stringify(data));
    if (r.data.status != 1) {
        if (r.data.result.includes('Unable to locate ContractCode')) {
            // waiting for etherscan backend and try again
            await sleep(15000);
            await publishSourceCode(contractname, contractaddress, sourceCode, compiled, constructorParams);
        } else {
            console.log(`Problem publishing ${contractname}:`, r.data);
        }
    } else {
        console.log(`Published ${contractname} sources on ${etherscanApiUrl}`);
    }
}

export async function deployGovernance(
    wallet,
    governorAddress,
    governanceCode,
    governanceSourceCode
    ) {
    try {
        let governance = await deployContract(wallet, governanceCode, [governorAddress], {
            gasLimit: 3000000,
        });
        console.log(`GOVERNANCE_ADDR=${governance.address}`);

        if (governanceSourceCode && process.env.FRANKLIN_ENV != 'dev') {
            publishSourceCode('Governance', governance.address, governanceSourceCode, governanceCode, [governorAddress]);;
        }

        return governance
    } catch (err) {
        console.log("Governance deploy error:" + err);
    }
}

export async function deployPriorityQueue(
    wallet,
    ownerAddress,
    priorityQueueCode,
    priorityQueueSourceCode
) {
    try {
        let priorityQueue = await deployContract(wallet, priorityQueueCode, [ownerAddress], {
            gasLimit: 5000000,
        });
        console.log(`PRIORITY_QUEUE_ADDR=${priorityQueue.address}`);

        if (priorityQueueSourceCode && process.env.FRANKLIN_ENV != 'dev') {
            publishSourceCode('PriorityQueue', priorityQueue.address, priorityQueueSourceCode, priorityQueueCode, [ownerAddress]);
        }

        return priorityQueue
    } catch (err) {
        console.log("Priority queue deploy error:" + err);
    }
}

export async function deployVerifier(
    wallet,
    verifierCode,
    verifierSourceCode
) {
    try {
        let verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 2000000,
        });
        console.log(`VERIFIER_ADDR=${verifier.address}`);

        if (verifierSourceCode && process.env.FRANKLIN_ENV != 'dev') {
            publishSourceCode('Verifier', verifier.address, verifierSourceCode, verifierCode, []);;
        }


        return verifier
    } catch (err) {
        console.log("Verifier deploy error:" + err);
    }
}

export async function deployFranklin(
    wallet,
    franklinCode,
    franklinSourceCode,
    governanceAddress,
    priorityQueueAddress,
    verifierAddress,
    genesisAddress,
    genesisRoot = ethers.constants.HashZero
) {
    try {
        let contract = await deployContract(
            wallet,
            franklinCode,
            [
                governanceAddress,
                verifierAddress,
                priorityQueueAddress,
                genesisAddress,
                genesisRoot,
            ],
            {
                gasLimit: 6600000,
            });
        console.log(`CONTRACT_ADDR=${contract.address}`);

        if (franklinSourceCode && process.env.FRANKLIN_ENV != 'dev') {
            publishSourceCode('Franklin', contract.address, franklinSourceCode, franklinCode, [
                        governanceAddress,
                        verifierAddress,
                        priorityQueueAddress,
                        genesisAddress,
                        genesisRoot,
                    ]);;
        }

        const priorityQueueContract = new ethers.Contract(priorityQueueAddress, priorityQueueContractCode.interface, wallet);
        await (await priorityQueueContract.changeFranklinAddress(contract.address)).wait();
        return contract
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
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        await (await governance.addToken(erc20.address)).wait();
        return erc20
    } catch (err) {
        console.log("Add token error:" + err);
    }
}

export async function addTestNotApprovedERC20Token(wallet) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, bigNumberify("1000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        return erc20
    } catch (err) {
        console.log("Add token error:" + err);
    }
}
