import { deployContract } from 'ethereum-waffle';
import { ethers } from 'ethers';
import { bigNumberify, parseEther } from "ethers/utils";
import Axios from "axios";
import * as qs from 'querystring';
import * as url from 'url';
import * as fs from 'fs';
import * as path from 'path';
import * as assert from 'assert';

export function abiRawEncode(args, vals) {
    return Buffer.from(ethers.utils.defaultAbiCoder.encode(args, vals).slice(2), 'hex');
}

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export const ERC20MintableContract = function () {
    let contract = require('openzeppelin-solidity/build/contracts/ERC20Mintable');
    contract.evm = { bytecode: contract.bytecode };
    contract.interface = contract.abi;
    return contract
}();

export const upgradeGatekeeperContractCode = require(`../build/UpgradeGatekeeper`);
export const franklinContractCode = require(`../build/Franklin`);
export const verifierContractCode = require(`../build/Verifier`);
export const governanceContractCode = require(`../build/Governance`);
export const proxyContractCode = require(`../build/Proxy`);

export const upgradeGatekeeperTestContractCode = require(`../build/UpgradeGatekeeperTest`);
export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');

import { ImportsFsEngine } from '@resolver-engine/imports-fs';
import { gatherSources } from '@resolver-engine/imports';

async function getSolidityInput(contractPath) {
    let input = await gatherSources([contractPath], process.cwd(), ImportsFsEngine());
    input = input.map(obj => ({...obj, url: obj.url.replace(`${process.cwd()}/`, '')}));

    let sources: { [s: string]: {} } = {};
    for (let file of input) {
        sources[file.url] = { content: file.source };
    }

    let config = require('../.waffle.json');
    let inputJSON = {
        language: "Solidity",
        sources,
        settings: {
            outputSelection: {
                "*": {
                    "*": [
                        "abi",
                        "evm.bytecode",
                        "evm.deployedBytecode"
                    ]
                }
            },
            ...config.compilerOptions
        }
    };

    return JSON.stringify(inputJSON, null, 2);
}

export class Deployer {
    bytecodes: any;
    addresses: any;
    deployTransactionHash: any;

    constructor(public wallet: ethers.Wallet, isTest: boolean) {
        this.bytecodes = {
            GovernanceTarget:    isTest ? governanceTestContractCode        : governanceContractCode,
            VerifierTarget:      isTest ? verifierTestContractCode          : verifierContractCode,
            FranklinTarget:      isTest ? franklinTestContractCode          : franklinContractCode,
            Governance:          proxyContractCode,
            Verifier:            proxyContractCode,
            Franklin:            proxyContractCode,
            UpgradeGatekeeper:   isTest ? upgradeGatekeeperTestContractCode : upgradeGatekeeperContractCode,
            ERC20:               ERC20MintableContract,
        };

        this.addresses = {
            GovernanceTarget: process.env.GOVERNANCE_TARGET_ADDR,
            VerifierTarget: process.env.VERIFIER_TARGET_ADDR,
            FranklinTarget: process.env.CONTRACT_TARGET_ADDR,
            Governance: process.env.GOVERNANCE_ADDR,
            Verifier: process.env.VERIFIER_ADDR,
            Franklin: process.env.CONTRACT_ADDR,
            UpgradeGatekeeper: process.env.UPGRADE_GATEKEEPER_ADDR,
            ERC20: process.env.TEST_ERC20,
        };

        this.deployTransactionHash = {
            Governance: process.env.GOVERNANCE_GENESIS_TX_HASH,
            Franklin: process.env.CONTRACT_GENESIS_TX_HASH,
        };
    }

    getDeployTransactionHash(name) {
        return this.deployTransactionHash[name];
    }

    getDeployedProxyContract(name) {
        return new ethers.Contract(
            this.addresses[name],
            this.bytecodes[name+"Target"].interface,
            this.wallet
        );
    }

    getDeployedContract(name): ethers.Contract {
        return new ethers.Contract(
            this.addresses[name],
            this.bytecodes[name].interface,
            this.wallet
        );
    }

    initializationArgs(contractName) {
        return {
            'Governance': [["address"], [this.wallet.address]],
            'Verifier': [[], []],
            'Franklin': [["address", "address", "address", "bytes32"], [
                this.addresses.Governance,
                this.addresses.Verifier,
                process.env.OPERATOR_FRANKLIN_ADDRESS,
                process.env.GENESIS_ROOT || ethers.constants.HashZero,
            ]],
        }[contractName];
    }
    encodedInitializationArgs(contractName) {
        let [initArgs, initArgsValues] = this.initializationArgs(contractName);
        return abiRawEncode(initArgs, initArgsValues);
    }
    constructorArgs(contractName) {
        return {
            'GovernanceTarget': [],
            'VerifierTarget': [],
            'FranklinTarget': [],
            'Governance': [this.addresses.GovernanceTarget, this.encodedInitializationArgs('Governance')],
            'Verifier': [this.addresses.VerifierTarget, this.encodedInitializationArgs('Verifier')],
            'Franklin': [this.addresses.FranklinTarget, this.encodedInitializationArgs('Franklin')],
            'UpgradeGatekeeper': [this.addresses.Franklin],
        }[contractName];
    }
    encodedConstructorArgs(contractName) {
        const args = this.constructorArgs(contractName);
        const iface = this.bytecodes[contractName].abi.filter(i => i.type === 'constructor');

        if (iface.length == 0) return null;

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
        const target = await deployContract(
            this.wallet,
            this.bytecodes.GovernanceTarget,
            this.constructorArgs('GovernanceTarget'),
            { gasLimit: 3000000, },
        );
        this.addresses.GovernanceTarget = target.address;

        const proxy = await deployContract(
            this.wallet,
            this.bytecodes.Governance,
            this.constructorArgs('Governance'),
            { gasLimit: 3000000, },
        );
        this.addresses.Governance = proxy.address;
        this.deployTransactionHash.Governance = proxy.deployTransaction.hash;
        return new ethers.Contract(proxy.address, this.bytecodes.GovernanceTarget.interface, this.wallet);
    }

    async deployVerifier() {
        const target = await deployContract(
            this.wallet,
            this.bytecodes.VerifierTarget,
            this.constructorArgs('VerifierTarget'),
            { gasLimit: 5000000 },
        );
        this.addresses.VerifierTarget = target.address;

        const proxy = await deployContract(
            this.wallet,
            this.bytecodes.Verifier,
            this.constructorArgs('Verifier'),
            { gasLimit: 3000000, },
        );
        this.addresses.Verifier = proxy.address;
        return new ethers.Contract(proxy.address, this.bytecodes.VerifierTarget.interface, this.wallet);
    }

    async deployFranklin() {
        const target = await deployContract(
            this.wallet,
            this.bytecodes.FranklinTarget,
            this.constructorArgs('FranklinTarget'),
            { gasLimit: 6500000, },
        );
        this.addresses.FranklinTarget = target.address;

        const proxy = await deployContract(
            this.wallet,
            this.bytecodes.Franklin,
            this.constructorArgs('Franklin'),
            { gasLimit: 3000000, },
        );
        this.addresses.Franklin = proxy.address;
        this.deployTransactionHash.Franklin = proxy.deployTransaction.hash;
        return new ethers.Contract(proxy.address, this.bytecodes.FranklinTarget.interface, this.wallet);
    }

    async deployUpgradeGatekeeper() {
        const contract = await deployContract(
            this.wallet,
            this.bytecodes.UpgradeGatekeeper,
            this.constructorArgs('UpgradeGatekeeper'),
            { gasLimit: 3000000, },
        );
        this.addresses.UpgradeGatekeeper = contract.address;

        const promises = [
            await this.getDeployedContract('Governance').transferMastership(contract.address),
            await this.getDeployedContract('Verifier').transferMastership(contract.address),
            await this.getDeployedContract('Franklin').transferMastership(contract.address),

            await contract.addUpgradeable(this.addresses['Governance']),
            await contract.addUpgradeable(this.addresses['Verifier']),
            await contract.addUpgradeable(this.addresses['Franklin']),
        ];

        await Promise.all(promises.map(tx => tx.wait()));

        return contract;
    }

    async addTestERC20Token(approve: "GovernanceApprove" | "GovernanceNotApprove") {
        assert(["GovernanceApprove", "GovernanceNotApprove"].includes(approve));
        let erc20 = await deployContract(
            this.wallet,
            this.bytecodes.ERC20,
            [],
            {
                gasLimit: 3000000,

            }
        );
        this.addresses.ERC20 = erc20.address;
        await erc20.mint(this.wallet.address, parseEther("3000000000"));
        if (approve == "GovernanceApprove") {
            const governance = this.getDeployedProxyContract('Governance');
            await governance.addToken(erc20.address);
        }
        return erc20;
    }

    async mintTestERC20Token(address, erc20?: ethers.Contract) {
        erc20 = erc20 || this.getDeployedContract("ERC20");
        const txCall = await erc20.mint(address, parseEther("3000000000"));
        await txCall.wait();
    }

    async setMoreTestValidators() {
        const mnemonics = [
            process.env.EXTRA_OPERATOR_MNEMONIC_1,
            process.env.EXTRA_OPERATOR_MNEMONIC_2,
        ];
        const governance = this.getDeployedProxyContract('Governance');
        for (const mnemonic of mnemonics) {
            const wallet = ethers.Wallet.fromMnemonic(mnemonic, "m/44'/60'/0'/0/1");
            await governance.setValidator(wallet.address, true).then(tx => tx.wait());
            console.log();
            console.log(`MNEMONIC="${mnemonic}"`);
            console.log(`OPERATOR_PRIVATE_KEY=${wallet.privateKey.slice(2)}`);
            console.log(`OPERATOR_ETH_ADDRESS=${wallet.address}`);
            console.log(`OPERATOR_FRANKLIN_ADDRESS=${wallet.address}`);
            console.log();
        }
    }

    async setGovernanceValidator() {
        const governance = await this.getDeployedProxyContract('Governance');
        await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);
    }

    async sendEthToTestWallets() {
        for (let i = 0; i < 10; ++i) {
            const to = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).address;
            await this.wallet.sendTransaction({ to, value: parseEther("100") });
            console.log(`sending ETH to ${to}`);
        }
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

    async publishSourceCodeToEtherscan(contractname) {
        const contractPath = `contracts/${contractname}.sol`;
        const sourceCode = await getSolidityInput(contractPath);

        const network = process.env.ETH_NETWORK;
        const etherscanApiUrl = network === 'mainnet' ? 'https://api.etherscan.io/api' : `https://api-${network}.etherscan.io/api`;

        const constructorArguments = this.encodedConstructorArgs(contractname);
        const contractaddress = this.addresses[contractname];

        let data = {
            apikey:             process.env.ETHERSCAN_API_KEY,  // A valid API-Key is required
            module:             'contract',                     // Do not change
            action:             'verifysourcecode',             // Do not change
            contractaddress,                                    // Contract Address starts with 0x...
            sourceCode,                                         // Contract Source Code (Flattened if necessary)
            codeformat:         'solidity-standard-json-input',
            contractname:       `${contractPath}:${contractname}`,
            compilerversion:    'v0.5.16+commit.9c3226ce',      // see http://etherscan.io/solcversions for list of support versions
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
                    await this.publishSourceCodeToEtherscan(contractname);
                }
            } else {
                console.log(`Problem publishing ${contractname}:`, r.data);
            }
        } else {
            let status;
            let retriesLeft = 10;
            while (retriesLeft --> 0) {
                status = await Axios.get(`http://api.etherscan.io/api?module=contract&&action=checkverifystatus&&guid=${r.data.result}`).then(r => r.data);

                if (status.result.includes('Pending in queue') == false)
                    break;

                await sleep(5000);
            }

            console.log(`Published ${contractname} sources on https://${network}.etherscan.io/address/${contractaddress} with status`, status);
        }
    }
}
