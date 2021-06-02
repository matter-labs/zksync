import { deployContract } from 'ethereum-waffle';
import { ethers, Signer, providers } from 'ethers';
import { formatEther, Interface } from 'ethers/lib/utils';
import * as fs from 'fs';
import {
    encodeConstructorArgs,
    encodeProxyContstuctorArgs,
    publishAbiToTesseracts,
    publishSourceCodeToEtherscan
} from './publish-utils';
import {
    Governance,
    GovernanceFactory,
    UpgradeGatekeeper,
    UpgradeGatekeeperFactory,
    Verifier,
    VerifierFactory,
    ZkSync,
    ZkSyncFactory,
    ForcedExit,
    ForcedExitFactory,
    TokenGovernanceFactory,
    TokenGovernance
} from '../typechain';

export interface Contracts {
    governance;
    zkSync;
    verifier;
    proxy;
    upgradeGatekeeper;
    forcedExit;
    regenesisMultisig;
    nftFactory;
    additionalZkSync;
    tokenGovernance;
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
    ForcedExit: string;
    RegenesisMultisig: string;
    NFTFactory: string;
    AdditionalZkSync: string;
    TokenGovernance: string;
}

export interface DeployerConfig {
    deployWallet: ethers.Wallet;
    governorAddress?: string;
    verbose?: boolean;
    contracts?: Contracts;
}

export function readContractCode(name: string) {
    const fileName = name.split('/').pop();
    return JSON.parse(
        fs.readFileSync(`artifacts/cache/solpp-generated-contracts/${name}.sol/${fileName}.json`, { encoding: 'utf-8' })
    );
}

export function readProductionContracts(): Contracts {
    return {
        nftFactory: readContractCode('ZkSyncNFTFactory'),
        governance: readContractCode('Governance'),
        zkSync: readContractCode('ZkSync'),
        verifier: readContractCode('Verifier'),
        proxy: readContractCode('Proxy'),
        upgradeGatekeeper: readContractCode('UpgradeGatekeeper'),
        forcedExit: readContractCode('ForcedExit'),
        regenesisMultisig: readContractCode('RegenesisMultisig'),
        additionalZkSync: readContractCode('AdditionalZkSync'),
        tokenGovernance: readContractCode('TokenGovernance')
    };
}

export function deployedAddressesFromEnv(): DeployedAddresses {
    return {
        NFTFactory: process.env.CONTRACTS_NFT_FACTORY_ADDR,
        DeployFactory: process.env.CONTRACTS_DEPLOY_FACTORY_ADDR,
        Governance: process.env.CONTRACTS_GOVERNANCE_ADDR,
        GovernanceTarget: process.env.CONTRACTS_GOVERNANCE_TARGET_ADDR,
        UpgradeGatekeeper: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,
        Verifier: process.env.CONTRACTS_VERIFIER_ADDR,
        VerifierTarget: process.env.CONTRACTS_VERIFIER_TARGET_ADDR,
        ZkSync: process.env.CONTRACTS_CONTRACT_ADDR,
        ZkSyncTarget: process.env.CONTRACTS_CONTRACT_TARGET_ADDR,
        ForcedExit: process.env.CONTRACTS_FORCED_EXIT_ADDR,
        RegenesisMultisig: process.env.MISC_REGENESIS_MULTISIG_ADDRESS,
        AdditionalZkSync: process.env.MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS,
        TokenGovernance: process.env.CONTRACTS_LISTING_GOVERNANCE
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
        this.deployFactoryCode = readContractCode('DeployFactory');
        this.verbose = config.verbose != null ? config.verbose : false;
        this.addresses = deployedAddressesFromEnv();
        this.contracts = config.contracts != null ? config.contracts : readProductionContracts();
        this.governorAddress = config.governorAddress != null ? config.governorAddress : this.deployWallet.address;
    }

    public async deployGovernanceTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying governance target');
        }

        const govContract = await deployContract(this.deployWallet, this.contracts.governance, [], {
            gasLimit: 1500000,
            ...ethTxOptions
        });
        const govRec = await govContract.deployTransaction.wait();
        const govGasUsed = govRec.gasUsed;
        const gasPrice = govContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_GOVERNANCE_TARGET_ADDR=${govContract.address}`);
            console.log(
                `Governance target deployed, gasUsed: ${govGasUsed.toString()}, eth spent: ${formatEther(
                    govGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.GovernanceTarget = govContract.address;
    }

    public async deployVerifierTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying verifier target');
        }
        const verifierContract = await deployContract(this.deployWallet, this.contracts.verifier, [], {
            gasLimit: 8000000,
            ...ethTxOptions
        });
        const verRec = await verifierContract.deployTransaction.wait();
        const verGasUsed = verRec.gasUsed;
        const gasPrice = verifierContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_VERIFIER_TARGET_ADDR=${verifierContract.address}`);
            console.log(
                `Verifier target deployed, gasUsed: ${verGasUsed.toString()}, eth spent: ${formatEther(
                    verGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.VerifierTarget = verifierContract.address;
    }

    public async deployZkSyncTarget(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying zkSync target');
        }
        const zksContract = await deployContract(this.deployWallet, this.contracts.zkSync, [], {
            gasLimit: 6000000,
            ...ethTxOptions
        });
        const zksRec = await zksContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = zksContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_CONTRACT_TARGET_ADDR=${zksContract.address}`);
            console.log(
                `zkSync target deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.ZkSyncTarget = zksContract.address;
    }

    public async deployProxiesAndGatekeeper(ethTxOptions?: ethers.providers.TransactionRequest) {
        let genesis_root = process.env.CONTRACTS_GENESIS_ROOT;
        const deployFactoryContract = await deployContract(
            this.deployWallet,
            this.deployFactoryCode,
            [
                this.addresses.GovernanceTarget,
                this.addresses.VerifierTarget,
                this.addresses.ZkSyncTarget,
                genesis_root,
                process.env.ETH_SENDER_SENDER_OPERATOR_COMMIT_ETH_ADDR,
                this.governorAddress,
                process.env.CHAIN_STATE_KEEPER_FEE_ACCOUNT_ADDR
            ],
            { gasLimit: 6000000, ...ethTxOptions }
        );
        const deployFactoryTx = await deployFactoryContract.deployTransaction.wait();
        const deployFactoryInterface = new Interface(this.deployFactoryCode.abi);

        for (const log of deployFactoryTx.logs) {
            try {
                const parsedLog = deployFactoryInterface.parseLog(log);
                if (parsedLog) {
                    this.addresses.Governance = parsedLog.args.governance;
                    this.addresses.ZkSync = parsedLog.args.zksync;
                    this.addresses.Verifier = parsedLog.args.verifier;
                    this.addresses.UpgradeGatekeeper = parsedLog.args.gatekeeper;
                }
            } catch (_) {}
        }
        const txHash = deployFactoryTx.transactionHash;
        const gasUsed = deployFactoryTx.gasUsed;
        const gasPrice = deployFactoryContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_DEPLOY_FACTORY_ADDR=${deployFactoryContract.address}`);
            console.log(`CONTRACTS_GOVERNANCE_ADDR=${this.addresses.Governance}`);
            console.log(`CONTRACTS_CONTRACT_ADDR=${this.addresses.ZkSync}`);
            console.log(`CONTRACTS_VERIFIER_ADDR=${this.addresses.Verifier}`);
            console.log(`CONTRACTS_UPGRADE_GATEKEEPER_ADDR=${this.addresses.UpgradeGatekeeper}`);
            console.log(`CONTRACTS_GENESIS_TX_HASH=${txHash}`);
            console.log(
                `Deploy finished, gasUsed: ${gasUsed.toString()}, eth spent: ${formatEther(gasUsed.mul(gasPrice))}`
            );
        }
    }

    public async deployNFTFactory(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying NFT FACTORY contract');
        }
        const name = process.env.NFT_FACTORY_NAME;
        const symbol = process.env.NFT_FACTORY_SYMBOL;

        const nftFactoryContarct = await deployContract(
            this.deployWallet,
            this.contracts.nftFactory,
            [name, symbol, this.addresses.ZkSync],
            {
                gasLimit: 6000000,
                ...ethTxOptions
            }
        );
        const zksRec = await nftFactoryContarct.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = nftFactoryContarct.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_NFT_FACTORY_ADDR=${nftFactoryContarct.address}`);
            console.log(
                `NFT Factory contract deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.NFTFactory = nftFactoryContarct.address;
        await this.governanceContract(this.deployWallet).setDefaultNFTFactory(nftFactoryContarct.address);
    }

    public async deployTokenGovernance(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying Token Governance contract');
        }

        const governance = this.addresses.Governance;
        const listingFeeToken = process.env.MISC_LISTING_FEE_TOKEN;
        const listingFee = process.env.MISC_LISTING_FEE;
        const listingCap = process.env.MISC_LISTING_CAP;
        const treasury = process.env.MISC_LISTING_TREASURY;

        const tokenGovernanceContract = await deployContract(
            this.deployWallet,
            this.contracts.tokenGovernance,
            [governance, listingFeeToken, listingFee, listingCap, treasury],
            {
                gasLimit: 6000000,
                ...ethTxOptions
            }
        );
        const zksRec = await tokenGovernanceContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = tokenGovernanceContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`\nCONTRACTS_LISTING_GOVERNANCE=${tokenGovernanceContract.address}\n`);
            console.log(
                `Token governance contract deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.TokenGovernance = tokenGovernanceContract.address;
        await this.governanceContract(this.deployWallet).changeTokenGovernance(tokenGovernanceContract.address);
    }

    public async deployForcedExit(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying ForcedExit contract');
        }

        // Choose the this.deployWallet.address as the default receiver if the
        // FORCED_EXIT_REQUESTS_SENDER_ACCOUNT_ADDRESS is not present
        const receiver = process.env.FORCED_EXIT_REQUESTS_SENDER_ACCOUNT_ADDRESS || this.deployWallet.address;

        const forcedExitContract = await deployContract(
            this.deployWallet,
            this.contracts.forcedExit,
            [this.deployWallet.address, receiver],
            {
                gasLimit: 6000000,
                ...ethTxOptions
            }
        );
        const zksRec = await forcedExitContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = forcedExitContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`CONTRACTS_FORCED_EXIT_ADDR=${forcedExitContract.address}`);
            console.log(
                `ForcedExit contract deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.ForcedExit = forcedExitContract.address;
    }

    public async deployAdditionalZkSync(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying Additional Zksync contract');
        }

        const additionalZkSyncContract = await deployContract(this.deployWallet, this.contracts.additionalZkSync, [], {
            gasLimit: 6000000,
            ...ethTxOptions
        });
        const zksRec = await additionalZkSyncContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = additionalZkSyncContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS=${additionalZkSyncContract.address}`);
            console.log(
                `Additiinal zkSync contract deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.RegenesisMultisig = additionalZkSyncContract.address;
    }

    public async deployRegenesisMultisig(ethTxOptions?: ethers.providers.TransactionRequest) {
        if (this.verbose) {
            console.log('Deploying Regenesis Multisig contract');
        }

        const regenesisMultisigContract = await deployContract(
            this.deployWallet,
            this.contracts.regenesisMultisig,
            [process.env.MISC_REGENESIS_THRESHOLD],
            {
                gasLimit: 6000000,
                ...ethTxOptions
            }
        );
        const zksRec = await regenesisMultisigContract.deployTransaction.wait();
        const zksGasUsed = zksRec.gasUsed;
        const gasPrice = regenesisMultisigContract.deployTransaction.gasPrice;
        if (this.verbose) {
            console.log(`MISC_REGENESIS_MULTISIG_ADDRESS=${regenesisMultisigContract.address}`);
            console.log(
                `Regenesis Multisig contract deployed, gasUsed: ${zksGasUsed.toString()}, eth spent: ${formatEther(
                    zksGasUsed.mul(gasPrice)
                )}`
            );
        }
        this.addresses.RegenesisMultisig = regenesisMultisigContract.address;
    }

    public async publishSourcesToTesseracts() {
        console.log('Publishing ABI for UpgradeGatekeeper');
        await publishAbiToTesseracts(this.addresses.UpgradeGatekeeper, this.contracts.upgradeGatekeeper);
        console.log('Publishing ABI for ZkSync (proxy)');
        await publishAbiToTesseracts(this.addresses.ZkSync, this.contracts.zkSync);
        console.log('Publishing ABI for Verifier (proxy)');
        await publishAbiToTesseracts(this.addresses.Verifier, this.contracts.verifier);
        console.log('Publishing ABI for Governance (proxy)');
        await publishAbiToTesseracts(this.addresses.Governance, this.contracts.governance);
        console.log('Publishing ABI for ForcedExit');
        await publishAbiToTesseracts(this.addresses.ForcedExit, this.contracts.forcedExit);
    }

    public async publishSourcesToEtherscan() {
        console.log('Publishing sourcecode for UpgradeGatekeeper', this.addresses.UpgradeGatekeeper);
        await publishSourceCodeToEtherscan(
            this.addresses.UpgradeGatekeeper,
            'UpgradeGatekeeper',
            encodeConstructorArgs(this.contracts.upgradeGatekeeper, [this.addresses.ZkSync])
        );

        console.log('Publishing sourcecode for ZkSyncTarget', this.addresses.ZkSyncTarget);
        await publishSourceCodeToEtherscan(this.addresses.ZkSyncTarget, 'ZkSync', '');
        console.log('Publishing sourcecode for GovernanceTarget', this.addresses.GovernanceTarget);
        await publishSourceCodeToEtherscan(this.addresses.GovernanceTarget, 'Governance', '');
        console.log('Publishing sourcecode for VerifierTarget', this.addresses.VerifierTarget);
        await publishSourceCodeToEtherscan(this.addresses.VerifierTarget, 'Verifier', '');

        console.log('Publishing sourcecode for ZkSync (proxy)', this.addresses.ZkSync);
        await publishSourceCodeToEtherscan(
            this.addresses.ZkSync,
            'Proxy',
            encodeProxyContstuctorArgs(
                this.contracts.proxy,
                this.addresses.ZkSyncTarget,
                [this.addresses.Governance, this.addresses.Verifier, process.env.CONTRACTS_GENESIS_ROOT],
                ['address', 'address', 'bytes32']
            )
        );

        console.log('Publishing sourcecode for Verifier (proxy)', this.addresses.Verifier);
        await publishSourceCodeToEtherscan(
            this.addresses.Verifier,
            'Proxy',
            encodeProxyContstuctorArgs(this.contracts.proxy, this.addresses.VerifierTarget, [], [])
        );

        console.log('Publishing sourcecode for Governance (proxy)', this.addresses.Governance);
        await publishSourceCodeToEtherscan(
            this.addresses.Governance,
            'Proxy',
            encodeProxyContstuctorArgs(
                this.contracts.proxy,
                this.addresses.GovernanceTarget,
                [this.addresses.DeployFactory],
                ['address']
            )
        );

        console.log('Publishing sourcecode for ForcedExit', this.addresses.ForcedExit);
        await publishSourceCodeToEtherscan(this.addresses.ForcedExit, 'ForcedExit', '');
    }

    public async deployAll(ethTxOptions?: ethers.providers.TransactionRequest) {
        await this.deployZkSyncTarget(ethTxOptions);
        await this.deployGovernanceTarget(ethTxOptions);
        await this.deployVerifierTarget(ethTxOptions);
        await this.deployProxiesAndGatekeeper(ethTxOptions);
        await this.deployForcedExit(ethTxOptions);
        await this.deployNFTFactory(ethTxOptions);
    }

    public governanceContract(signerOrProvider: Signer | providers.Provider): Governance {
        return GovernanceFactory.connect(this.addresses.Governance, signerOrProvider);
    }

    public tokenGovernanceContract(signerOrProvider: Signer | providers.Provider): TokenGovernance {
        return TokenGovernanceFactory.connect(this.addresses.TokenGovernance, signerOrProvider);
    }

    public zkSyncContract(signerOrProvider: Signer | providers.Provider): ZkSync {
        return ZkSyncFactory.connect(this.addresses.ZkSync, signerOrProvider);
    }

    public verifierContract(signerOrProvider: Signer | providers.Provider): Verifier {
        return VerifierFactory.connect(this.addresses.Verifier, signerOrProvider);
    }

    public upgradeGatekeeperContract(signerOrProvider: Signer | providers.Provider): UpgradeGatekeeper {
        return UpgradeGatekeeperFactory.connect(this.addresses.UpgradeGatekeeper, signerOrProvider);
    }

    public forcedExitContract(signerOrProvider: Signer | providers.Provider): ForcedExit {
        return ForcedExitFactory.connect(this.addresses.ForcedExit, signerOrProvider);
    }
}
