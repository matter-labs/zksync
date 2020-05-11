import {deployContract} from "ethereum-waffle";
import {ethers} from "ethers";
import {formatEther, Interface, parseEther} from "ethers/utils";
import * as fs from "fs";
import {
    encodeConstructorArgs,
    encodeProxyContstuctorArgs,
    publishAbiToTesseracts,
    publishSourceCodeToEtherscan,
} from "./publish-utils";

type ContractName = "Governance" | "ZkSync" | "Verifier" | "Proxy" | "UpgradeGatekeeper";

export interface Contracts {
    governance;
    zkSync;
    verifier;
    proxy;
    upgradeGatekeeper;
}

export interface DeployedAddresses {
    Governance: string;
    GovernanceTarget: string;
    UpgradeGatekeeper: string;
    Verifier: string;
    VerifierTarget: string;
    ZkSync: string;
    ZkSyncTarget: string;
    DeployFactory: string;
}

export interface DeployerConfig {
    deployWallet: ethers.Wallet;
    governorAddress?: string;
    verbose?: boolean;
    contracts?: Contracts;
}

export function readContractCode(name: string) {
    if (name === "TEST-ERC20") {
        const contract = require("openzeppelin-solidity/build/contracts/ERC20Mintable");
        contract.evm = {bytecode: contract.bytecode};
        contract.interface = contract.abi;
        return contract;
    } else {
        return JSON.parse(fs.readFileSync(`build/${name}.json`, {encoding: "utf-8"}));
    }
}

export function readProductionContracts(): Contracts {
    return {
        governance: readContractCode("Governance"),
        zkSync: readContractCode("ZkSync"),
        verifier: readContractCode("Verifier"),
        proxy: readContractCode("Proxy"),
        upgradeGatekeeper: readContractCode("UpgradeGatekeeper"),
    };
}

export function readTestContracts(): Contracts {
    return {
        governance: readContractCode("GovernanceTest"),
        zkSync: readContractCode("ZkSyncTest"),
        verifier: readContractCode("VerifierTest"),
        proxy: readContractCode("Proxy"),
        upgradeGatekeeper: readContractCode("UpgradeGatekeeperTest"),
    };
}

export function deployedAddressesFromEnv(): DeployedAddresses {
    return {
        DeployFactory: process.env.DEPLOY_FACTORY_ADDR,
        Governance: process.env.GOVERNANCE_ADDR,
        GovernanceTarget: process.env.GOVERNANCE_TARGET_ADDR,
        UpgradeGatekeeper: process.env.UPGRADE_GATEKEEPER_ADDR,
        Verifier: process.env.VERIFIER_ADDR,
        VerifierTarget: process.env.VERIFIER_TARGET_ADDR,
        ZkSync: process.env.CONTRACT_ADDR,
        ZkSyncTarget: process.env.CONTRACT_TARGET_ADDR,
    };
}

export class Deployer {
    public addresses: DeployedAddresses;
    private deployWallet;
    private deployFactoryCode;
    private verbose;
    private contracts: Contracts;
    private governorAddress: string;

    constructor(config: DeployerConfig) {
        this.deployWallet = config.deployWallet;
        this.deployFactoryCode = readContractCode("DeployFactory");
        this.verbose = config.verbose != null ? config.verbose : false;
        this.addresses = deployedAddressesFromEnv();
        this.contracts = config.contracts != null ? config.contracts : readProductionContracts();
        this.governorAddress = config.governorAddress != null ? config.governorAddress : this.deployWallet.address;
    }

    public async deployGovernanceTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log("Deploying governance target");
        }
        const govContract = await deployContract(
            this.deployWallet,
            this.contracts.governance, [],
            {gasLimit: 600000, ...ethTxOptions},
        );
        const govRec = await govContract.deployTransaction.wait();
        const govGasUsed = govRec.gasUsed;
        const gasPrice = govContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`GOVERNANCE_TARGET_ADDR=${govContract.address}`);
            console.log(`Governance target deployed, gasUsed: ${govGasUsed.toString()}, eth spent: ${formatEther(govGasUsed.mul(gasPrice))}`);
        }
        this.addresses.GovernanceTarget = govContract.address;
    }

    public async deployVerifierTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log("Deploying verifier target");
        }
        const verifierContract = await deployContract(
            this.deployWallet,
            this.contracts.verifier, [],
            {gasLimit: 4000000, ...ethTxOptions},
        );
        const verRec = await verifierContract.deployTransaction.wait();
        const verGasUsed = verRec.gasUsed;
        const gasPrice = verifierContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`VERIFIER_TARGET_ADDR=${verifierContract.address}`);
            console.log(`Verifier target deployed, gasUsed: ${verGasUsed.toString()}, eth spent: ${formatEther(verGasUsed.mul(gasPrice))}`);
        }
        this.addresses.VerifierTarget = verifierContract.address;
    }

    public async deployZkSyncTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log("Deploying zkSync target");
        }
        const zksContract = await deployContract(
            this.deployWallet,
            this.contracts.zkSync, [],
            {gasLimit: 5000000, ...ethTxOptions},
        );
        const zksRec = await zksContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = zksContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACT_TARGET_ADDR=${zksContract.address}`);
            console.log(`zkSync target deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(zksGasUsed.mul(gasPrice))}`);
        }
        this.addresses.ZkSyncTarget = zksContract.address;
    }

    public async deployProxiesAndGatekeeper(ethTxOptions?: ethers.providers.TransactionRequest) {
        const deployFactoryContract = await deployContract(
            this.deployWallet,
            this.deployFactoryCode, [this.addresses.GovernanceTarget, this.addresses.VerifierTarget,
                this.addresses.ZkSyncTarget, process.env.GENESIS_ROOT, process.env.OPERATOR_ETH_ADDRESS,
                this.governorAddress,
            ],
            {gasLimit: 5000000, ...ethTxOptions},
        );
        const deployFactoryTx = await deployFactoryContract.deployTransaction.wait();
        const deployFactoryInterface = new Interface(this.deployFactoryCode.interface);
        for (const log of deployFactoryTx.logs) {
            const parsedLog = deployFactoryInterface.parseLog(log);
            if (parsedLog) {
                this.addresses.Governance = parsedLog.values.governance;
                this.addresses.ZkSync = parsedLog.values.zksync;
                this.addresses.Verifier = parsedLog.values.verifier;
                this.addresses.UpgradeGatekeeper = parsedLog.values.gatekeeper;
            }
        }
        const govGasUsed = deployFactoryTx.gasUsed;
        const gasPrice = deployFactoryContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`DEPLOY_FACTORY_ADDR=${deployFactoryContract.address}`);
            console.log(`GOVERNANCE_ADDR=${this.addresses.Governance}`);
            console.log(`CONTRACT_ADDR=${this.addresses.ZkSync}`);
            console.log(`VERIFIER_ADDR=${this.addresses.Verifier}`);
            console.log(`UPGRADE_GATEKEEPER_ADDR=${this.addresses.UpgradeGatekeeper}`);
            console.log(`Deploy finished, gasUsed: ${govGasUsed.toString()}, eth spent: ${formatEther(govGasUsed.mul(gasPrice))}`);
        }
    }

    public async publishSourcesToTesseracts() {
        console.log("Publishing ABI for UpgradeGatekeeper");
        await publishAbiToTesseracts(this.addresses.UpgradeGatekeeper, this.contracts.upgradeGatekeeper);
        console.log("Publishing ABI for ZkSync (proxy)");
        await publishAbiToTesseracts(this.addresses.ZkSync, this.contracts.zkSync);
        console.log("Publishing ABI for Verifier (proxy)");
        await publishAbiToTesseracts(this.addresses.Verifier, this.contracts.verifier);
        console.log("Publishing ABI for Governance (proxy)");
        await publishAbiToTesseracts(this.addresses.Governance, this.contracts.governance);
    }

    public async publishSourcesToEtherscan() {
        console.log("Publishing sourcecode for UpgradeGatekeeper");
        await publishSourceCodeToEtherscan(this.addresses.UpgradeGatekeeper, "UpgradeGatekeeper",
            encodeConstructorArgs(this.contracts.upgradeGatekeeper, [this.addresses.ZkSync]));

        console.log("Publishing sourcecode for ZkSyncTarget");
        await publishSourceCodeToEtherscan(this.addresses.ZkSyncTarget, "ZkSync", "");
        console.log("Publishing sourcecode for GovernanceTarget");
        await publishSourceCodeToEtherscan(this.addresses.GovernanceTarget, "Governance", "");
        console.log("Publishing sourcecode for VerifierTarget");
        await publishSourceCodeToEtherscan(this.addresses.VerifierTarget, "Verifier", "");

        console.log("Publishing sourcecode for ZkSync (proxy)");
        await publishSourceCodeToEtherscan(this.addresses.ZkSync, "Proxy",
            encodeProxyContstuctorArgs(this.contracts.proxy, this.addresses.ZkSyncTarget,
                [this.addresses.Governance, this.addresses.Verifier, process.env.GENESIS_ROOT],
                ["address", "address", "bytes32"]));

        console.log("Publishing sourcecode for Verifier (proxy)");
        await publishSourceCodeToEtherscan(this.addresses.Verifier, "Proxy",
            encodeProxyContstuctorArgs(this.contracts.proxy, this.addresses.VerifierTarget, [], []));

        console.log("Publishing sourcecode for Governance (proxy)");
        await publishSourceCodeToEtherscan(this.addresses.Governance, "Proxy",
            encodeProxyContstuctorArgs(this.contracts.proxy, this.addresses.GovernanceTarget,
                [this.addresses.DeployFactory], ["address"]));
    }

    public async deployAll() {
        await this.deployZkSyncTarget();
        await this.deployGovernanceTarget();
        await this.deployVerifierTarget();
        await this.deployProxiesAndGatekeeper();
    }
}
