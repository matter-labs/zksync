import { deployContract } from 'ethereum-waffle';
import { ethers } from 'ethers';
import { bigNumberify, parseEther } from "ethers/utils";
import Axios from "axios";
import * as qs from 'querystring';
import * as url from 'url';
import * as fs from 'fs';
import * as path from 'path';

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = { bytecode: contract.bytecode };
    return contract
}();

export const franklinContractCode = require(`../build/Franklin`);
export const verifierContractCode = require(`../build/Verifier`);
export const governanceContractCode = require(`../build/Governance`);
export const priorityQueueContractCode = require(`../build/PriorityQueue`);

export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');
export const priorityQueueTestContractCode = require('../build/PriorityQueueTest');

export class Deployer {
    bytecodes: any;
    addresses: any;

    constructor(public wallet: ethers.Wallet, isTest: boolean) {
        this.bytecodes = {
            Governance: isTest ? governanceContractCode : governanceTestContractCode,
            PriorityQueue: isTest ? priorityQueueContractCode : priorityQueueTestContractCode,
            Verifier: isTest ? verifierContractCode : verifierTestContractCode,
            Franklin: isTest ? franklinContractCode : franklinTestContractCode,
        };

        this.addresses = {
            Governance: process.env.GOVERNANCE_ADDR,
            PriorityQueue: process.env.PRIORITY_QUEUE_ADDR,
            Verifier: process.env.VERIFIER_ADDR,
            Franklin: process.env.CONTRACT_ADDR,
        };
    }

    getDeployedContract(name) {
        return new ethers.Contract(
            this.addresses[name],
            this.bytecodes[name].interface,
            this.wallet
        );
    }

    constructorArgs(contractName) {
        return {
            'Governance': [this.wallet.address],
            'PriorityQueue': [this.addresses.Governance],
            'Verifier': [],
            'Franklin': [
                this.addresses.Governance,
                this.addresses.Verifier,
                this.addresses.PriorityQueue,
                process.env.OPERATOR_FRANKLIN_ADDRESS,
                process.env.GENESIS_ROOT || ethers.constants.HashZero,
            ]
        }[contractName];
    }
    encodedConstructorArgs(contractName) {
        const args = this.constructorArgs(contractName);
        const iface = this.bytecodes[contractName].abi.filter(i => i.type === 'constructor');
        return ethers
            .utils
            .defaultAbiCoder
            .encode(
                iface[0].inputs,
                args
            )
            .slice(2);
    }

    async deployGovernance() {
        const governance = await deployContract(
            this.wallet, 
            this.bytecodes.Governance, 
            this.constructorArgs('Governance'),
            { gasLimit: 3000000 }
        );
        console.log(`GOVERNANCE_GENESIS_TX_HASH=${governance.deployTransaction.hash}`);
        console.log(`GOVERNANCE_ADDR=${governance.address}`);
        this.addresses.Governance = governance.address;
        return governance;
    }

    async deployPriorityQueue() {
        let priorityQueue = await deployContract(
            this.wallet, 
            this.bytecodes.PriorityQueue, 
            this.constructorArgs('PriorityQueue'), 
            { gasLimit: 5000000 }
        );
        console.log(`PRIORITY_QUEUE_ADDR=${priorityQueue.address}`);
        this.addresses.PriorityQueue = priorityQueue.address;
        return priorityQueue;
    }

    async deployVerifier() {
        let verifier = await deployContract(
            this.wallet, 
            this.bytecodes.Verifier, 
            this.constructorArgs('Verifier'),
            { gasLimit: 2000000 }
        );
        console.log(`VERIFIER_ADDR=${verifier.address}`);
        this.addresses.Verifier = verifier.address;
        return verifier;
    }

    async deployFranklin() {
        let franklin = await deployContract(
            this.wallet,
            this.bytecodes.Franklin,
            this.constructorArgs('Franklin'),
            { gasLimit: 6000000}
        );
        console.log(`CONTRACT_GENESIS_TX_HASH=${franklin.deployTransaction.hash}`);
        console.log(`CONTRACT_ADDR=${franklin.address}`);
        this.addresses.Franklin = franklin.address;
        return franklin;
    }
    
    async postContractToTesseracts(contractName) {
        const address = this.addresses[contractName];
        const contractCode = this.bytecodes[contractName];

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
}

export async function addTestERC20Token(wallet, governance) {
    try {
        let erc20 = await deployContract(wallet, ERC20MintableContract, []);
        await erc20.mint(wallet.address, parseEther("3000000000"));
        console.log("TEST_ERC20=" + erc20.address);
        await (await governance.addToken(erc20.address)).wait();
        return erc20;
    } catch (err) {
        console.log("Add token error:" + err);
    }
}

export async function mintTestERC20Token(wallet, erc20) {
    try {
        const txCall = await erc20.mint(wallet.address, parseEther("3000000000"));
        await txCall.wait();
    } catch (err) {
        console.log("Mint token error:" + err);
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
