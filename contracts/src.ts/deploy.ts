import {deployContract} from 'ethereum-waffle';
import {ethers} from 'ethers';
import {BigNumber, formatEther, parseEther} from "ethers/utils";
import Axios from "axios";
import * as qs from 'querystring';
import * as assert from 'assert';

export function abiRawEncode(args, vals) {
    return Buffer.from(ethers.utils.defaultAbiCoder.encode(args, vals).slice(2), 'hex');
}

const sleep = async ms => await new Promise(resolve => setTimeout(resolve, ms));

export const proxyContractCode = require(`../build/Proxy`);

export const upgradeGatekeeperContractCode = require(`../build/UpgradeGatekeeper`);
export const franklinContractCode = require(`../build/Franklin`);
export const verifierContractCode = require(`../build/Verifier`);
export const governanceContractCode = require(`../build/Governance`);

export const upgradeGatekeeperTestContractCode = require(`../build/UpgradeGatekeeperTest`);
export const franklinTestContractCode = require('../build/FranklinTest');
export const verifierTestContractCode = require('../build/VerifierTest');
export const governanceTestContractCode = require('../build/GovernanceTest');

import {ImportsFsEngine} from '@resolver-engine/imports-fs';
import {gatherSources} from '@resolver-engine/imports';

async function getSolidityInput(contractPath) {
    let input = await gatherSources([contractPath], process.cwd(), ImportsFsEngine());
    input = input.map(obj => ({...obj, url: obj.url.replace(`${process.cwd()}/`, '')}));

    let sources: { [s: string]: {} } = {};
    for (let file of input) {
        sources[file.url] = {content: file.source};
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

    constructor(public deployerWallet: ethers.Wallet, isTest: boolean, public verbose: boolean) {
        this.bytecodes = {
            GovernanceTarget: isTest ? governanceTestContractCode : governanceContractCode,
            VerifierTarget: isTest ? verifierTestContractCode : verifierContractCode,
            FranklinTarget: isTest ? franklinTestContractCode : franklinContractCode,
            Governance: proxyContractCode,
            Verifier: proxyContractCode,
            Franklin: proxyContractCode,
            UpgradeGatekeeper: isTest ? upgradeGatekeeperTestContractCode : upgradeGatekeeperContractCode,
        };

        this.addresses = {
            GovernanceTarget: process.env.GOVERNANCE_TARGET_ADDR,
            VerifierTarget: process.env.VERIFIER_TARGET_ADDR,
            FranklinTarget: process.env.CONTRACT_TARGET_ADDR,
            Governance: process.env.GOVERNANCE_ADDR,
            Verifier: process.env.VERIFIER_ADDR,
            Franklin: process.env.CONTRACT_ADDR,
            UpgradeGatekeeper: process.env.UPGRADE_GATEKEEPER_ADDR,
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
            this.bytecodes[name + "Target"].interface,
            this.deployerWallet
        );
    }

    getDeployedContract(name): ethers.Contract {
        return new ethers.Contract(
            this.addresses[name],
            this.bytecodes[name].interface,
            this.deployerWallet
        );
    }

    initializationArgs(contractName) {
        return {
            'Governance': [["address"], [this.deployerWallet.address]],
            'Verifier': [[], []],
            'Franklin': [["address", "address", "address", "bytes32"], [
                this.addresses.Governance,
                this.addresses.Verifier,
                process.env.OPERATOR_ETH_ADDRESS,
                process.env.GENESIS_ROOT,
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

    async deployGovernanceTarget() {
        if (this.verbose) {
            console.log("Deploying Governance target");
        }
        const target = await deployContract(
            this.deployerWallet,
            this.bytecodes.GovernanceTarget,
            this.constructorArgs('GovernanceTarget'),
            {gasLimit: 3000000,},
        );
        const gasPrice = target.deployTransaction.gasPrice;
        const gasUsed = (await target.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`GOVERNANCE_TARGET_ADDR=${target.address}`);
            console.log(`Governance target deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
        this.addresses.GovernanceTarget = target.address;
    }

    async deployGovernance() {
        if (this.verbose) {
            console.log("Deploying Governance");
        }
        const proxy = await deployContract(
            this.deployerWallet,
            this.bytecodes.Governance,
            this.constructorArgs('Governance'),
            {gasLimit: 3000000,},
        );
        this.addresses.Governance = proxy.address;
        this.deployTransactionHash.Governance = proxy.deployTransaction.hash;
        const gasPrice = proxy.deployTransaction.gasPrice;
        const gasUsed = (await proxy.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`GOVERNANCE_GENESIS_TX_HASH=${this.deployTransactionHash.Governance}`);
            console.log(`GOVERNANCE_ADDR=${this.addresses.Governance}`);
            console.log(`Governance deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
        return new ethers.Contract(proxy.address, this.bytecodes.GovernanceTarget.interface, this.deployerWallet);
    }

    async deployVerifierTarget() {
        if (this.verbose) {
            console.log("Deploying Verifier target");
        }
        const target = await deployContract(
            this.deployerWallet,
            this.bytecodes.VerifierTarget,
            this.constructorArgs('VerifierTarget'),
            {gasLimit: 5000000},
        );
        this.addresses.VerifierTarget = target.address;
        const gasPrice = target.deployTransaction.gasPrice;
        const gasUsed = (await target.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`VERIFIER_TARGET_ADDR=${this.addresses.VerifierTarget}`);
            console.log(`Verifier target deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
    }

    async deployVerifier() {
        if (this.verbose) {
            console.log("Deploying Verifier");
        }
        const proxy = await deployContract(
            this.deployerWallet,
            this.bytecodes.Verifier,
            this.constructorArgs('Verifier'),
            {gasLimit: 3000000,},
        );
        this.addresses.Verifier = proxy.address;
        const gasPrice = proxy.deployTransaction.gasPrice;
        const gasUsed = (await proxy.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`VERIFIER_ADDR=${this.addresses.Verifier}`);
            console.log(`Verifier deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
        return new ethers.Contract(proxy.address, this.bytecodes.VerifierTarget.interface, this.deployerWallet);
    }


    async deployFranklinTarget() {
        if (this.verbose) {
            console.log("Deploying zkSync target");
        }
        const target = await deployContract(
            this.deployerWallet,
            this.bytecodes.FranklinTarget,
            this.constructorArgs('FranklinTarget'),
            {gasLimit: 6500000,},
        );
        this.addresses.FranklinTarget = target.address;
        const gasPrice = target.deployTransaction.gasPrice;
        const gasUsed = (await target.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`CONTRACT_TARGET_ADDR=${this.addresses.FranklinTarget}`);
            console.log(`zkSync target deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
    }

    async deployFranklin() {
        if (this.verbose) {
            console.log("Deploying zkSync contract");
        }
        const proxy = await deployContract(
            this.deployerWallet,
            this.bytecodes.Franklin,
            this.constructorArgs('Franklin'),
            {gasLimit: 3000000,},
        );
        this.addresses.Franklin = proxy.address;
        this.deployTransactionHash.Franklin = proxy.deployTransaction.hash;
        const gasPrice = proxy.deployTransaction.gasPrice;
        const gasUsed = (await proxy.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`CONTRACT_GENESIS_TX_HASH=${this.deployTransactionHash.Franklin}`);
            console.log(`CONTRACT_ADDR=${this.addresses.Franklin}`);
            console.log(`zkSync deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
        return new ethers.Contract(proxy.address, this.bytecodes.FranklinTarget.interface, this.deployerWallet);
    }

    async deployUpgradeGatekeeper() {
        if (this.verbose) {
            console.log("Deploying Upgrade Gatekeeper contract");
        }
        const contract = await deployContract(
            this.deployerWallet,
            this.bytecodes.UpgradeGatekeeper,
            this.constructorArgs('UpgradeGatekeeper'),
            {gasLimit: 3000000,},
        );
        this.addresses.UpgradeGatekeeper = contract.address;
        const gasPrice = contract.deployTransaction.gasPrice;
        const gasUsed = (await contract.deployTransaction.wait()).gasUsed;
        if (this.verbose) {
            console.log(`UPGRADE_GATEKEEPER_ADDR=${this.addresses.UpgradeGatekeeper}`);
            console.log(`Upgrade Gatekeeper deployed, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`);
        }
        return contract;
    }

    async transferMastershipToGatekeeper() {
        if (this.verbose) {
            console.log("Transfering mastership of contracts to Upgrade Gatekeeper");
        }
        const upgradeGatekeeper = new ethers.Contract(
            this.addresses['UpgradeGatekeeper'],
            this.bytecodes['UpgradeGatekeeper'].interface,
            this.deployerWallet);

        for (const contractName of ['Governance', 'Verifier', 'Franklin']) {
            if (this.verbose) {
                console.log(`Transferring ${contractName} mastership`);
            }
            let tx = await this.getDeployedContract(contractName).transferMastership(this.addresses.UpgradeGatekeeper);
            let receipt = await tx.wait();
            let ethUsed = tx.gasPrice.mul(receipt.gasUsed);
            tx = await upgradeGatekeeper.addUpgradeable(this.addresses[contractName]);
            receipt = await tx.wait();
            ethUsed = ethUsed.add(tx.gasPrice.mul(receipt.gasUsed));
            if (this.verbose) {
                console.log(`Done Transferring ${contractName} mastership, total eth spent: ${formatEther(ethUsed)}`);
            }
        }
    }

    async setGovernanceValidator() {
        if (this.verbose) {
            console.log("Setting operator as validator");
        }
        const governance = await this.getDeployedProxyContract('Governance');
        const tx = await governance.setValidator(process.env.OPERATOR_ETH_ADDRESS, true);
        const receipt = await tx.wait();
        const ethUsed = tx.gasPrice.mul(receipt.gasUsed);
        if (this.verbose) {
            console.log(`Done Setting operator as validator, gasUsed: ${receipt.gasUsed.toString()} eth spent: ${formatEther(ethUsed)}`);
        }
    }

    // async sendEthToTestWallets() {
    //     for (let i = 0; i < 10; ++i) {
    //         const to = ethers.Wallet.fromMnemonic(process.env.TEST_MNEMONIC, "m/44'/60'/0'/0/" + i).address;
    //         await this.wallet.sendTransaction({to, value: parseEther("100")});
    //         console.log(`sending ETH to ${to}`);
    //     }
    // }

    // async postContractToTesseracts(contractName) {
    //     const address = this.addresses[contractName];
    //     const contractCode = this.bytecodes[contractName];
    //
    //     let req = {
    //         contract_source: JSON.stringify(contractCode.abi),
    //         contract_compiler: "abi-only",
    //         contract_name: contractName,
    //         contract_optimized: false
    //     };
    //
    //     const config = {
    //         headers: {
    //             'Content-Type': 'application/x-www-form-urlencoded'
    //         }
    //     };
    //     await Axios.post(`http://localhost:8000/${address}/contract`, qs.stringify(req), config);
    // }
    //
    // async publishSourceCodeToEtherscan(contractname) {
    //     const contractPath = `contracts/${contractname}.sol`;
    //     const sourceCode = await getSolidityInput(contractPath);
    //
    //     const network = process.env.ETH_NETWORK;
    //     const etherscanApiUrl = network === 'mainnet' ? 'https://api.etherscan.io/api' : `https://api-${network}.etherscan.io/api`;
    //
    //     const constructorArguments = this.encodedConstructorArgs(contractname);
    //     const contractaddress = this.addresses[contractname];
    //
    //     let data = {
    //         apikey: process.env.ETHERSCAN_API_KEY,  // A valid API-Key is required
    //         module: 'contract',                     // Do not change
    //         action: 'verifysourcecode',             // Do not change
    //         contractaddress,                                    // Contract Address starts with 0x...
    //         sourceCode,                                         // Contract Source Code (Flattened if necessary)
    //         codeformat: 'solidity-standard-json-input',
    //         contractname: `${contractPath}:${contractname}`,
    //         compilerversion: 'v0.5.16+commit.9c3226ce',      // see http://etherscan.io/solcversions for list of support versions
    //         constructorArguements: constructorArguments         // if applicable. How nice, they have a typo in their api
    //     };
    //
    //     let r = await Axios.post(etherscanApiUrl, qs.stringify(data));
    //     let retriesLeft = 20;
    //     if (r.data.status != 1) {
    //         if (r.data.result.includes('Unable to locate ContractCode')) {
    //             // waiting for etherscan backend and try again
    //             await sleep(15000);
    //             if (retriesLeft > 0) {
    //                 --retriesLeft;
    //                 await this.publishSourceCodeToEtherscan(contractname);
    //             }
    //         } else {
    //             console.log(`Problem publishing ${contractname}:`, r.data);
    //         }
    //     } else {
    //         let status;
    //         let retriesLeft = 10;
    //         while (retriesLeft-- > 0) {
    //             status = await Axios.get(`http://api.etherscan.io/api?module=contract&&action=checkverifystatus&&guid=${r.data.result}`).then(r => r.data);
    //
    //             if (status.result.includes('Pending in queue') == false)
    //                 break;
    //
    //             await sleep(5000);
    //         }
    //
    //         console.log(`Published ${contractname} sources on https://${network}.etherscan.io/address/${contractaddress} with status`, status);
    //     }
    // }
}

export async function deployBySteps(deployWallet: ethers.Wallet, deployStep: number | "all", test: boolean, verbose: boolean) {
    const deployer = new Deployer(deployWallet, test, verbose);

    if (deployStep === 0 || deployStep === "all") {
        await deployer.deployGovernanceTarget();

    }
    if (deployStep === 1 || deployStep === "all") {
        await deployer.deployGovernance();
        console.log("\n");

    }
    if (deployStep === 2 || deployStep === "all") {
        await deployer.deployVerifierTarget();

    }
    if (deployStep === 3 || deployStep === "all") {
        await deployer.deployVerifier();
        console.log("\n");

    }
    if (deployStep === 4 || deployStep === "all") {
        await deployer.deployFranklinTarget();

    }
    if (deployStep === 5 || deployStep === "all") {
        await deployer.deployFranklin();
        console.log("\n");

    }
    if (deployStep === 6 || deployStep === "all") {
        await deployer.deployUpgradeGatekeeper();
        console.log("\n");

    }
    if (deployStep === 7 || deployStep === "all") {
        await deployer.transferMastershipToGatekeeper();
        console.log("\n");

    }
    if (deployStep === 8 || deployStep === "all") {
        await deployer.setGovernanceValidator();
    }
}

