import {deployContract} from 'ethereum-waffle';
import {ethers} from 'ethers';
import {bigNumberify} from "ethers/utils";
import Axios from "axios";
const qs = require('querystring');

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = {bytecode: contract.bytecode};
    return contract
}();
export const franklinContractCode = require('../build/FranklinTest');
export const verifierContractCode = require('../build/VerifierTest');
export const governanceContractCode = require('../build/Governance');
export const priorityQueueContractCode = require('../build/PriorityQueueTest');

export async function deployGovernance(
    wallet,
    governorAddress = wallet.address,
    governanceCode = governanceContractCode
    ) {
    try {
        let governance = await deployContract(wallet, governanceCode, [governorAddress], {
            gasLimit: 3000000,
        });
        console.log(`GOVERNANCE_ADDR=${governance.address}`);

        return governance
    } catch (err) {
        console.log("Governance deploy error:" + err);
    }
}

export async function deployPriorityQueue(
    wallet,
    ownerAddress = wallet.address,
    priorityQueueCode = priorityQueueContractCode
    ) {
    try {
        let priorityQueue = await deployContract(wallet, priorityQueueCode, [ownerAddress], {
            gasLimit: 5000000,
        });
        console.log(`PRIORITY_QUEUE_ADDR=${priorityQueue.address}`);

        return priorityQueue
    } catch (err) {
        console.log("Priority queu deploy error:" + err);
    }
}

export async function deployVerifier(
    wallet,
    verifierCode = verifierContractCode
    ) {
    try {
        let verifier = await deployContract(wallet, verifierCode, [], {
            gasLimit: 2000000,
        });
        console.log(`VERIFIER_ADDR=${verifier.address}`);

        return verifier
    } catch (err) {
        console.log("Verifier deploy error:" + err);
    }
}

export async function deployFranklin(
    wallet,
    governanceAddress,
    priorityQueueAddress,
    verifierAddress,
    genesisRoot = ethers.constants.HashZero,
    franklinCode = franklinContractCode
    ) {
    try {
        let contract = await deployContract(
            wallet,
            franklinCode,
            [
                governanceAddress,
                verifierAddress,
                priorityQueueAddress,
                genesisRoot,
            ],
        {
            gasLimit: 6600000,
        });
        console.log(`CONTRACT_ADDR=${contract.address}`);

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
