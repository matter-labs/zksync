import '@nomiclabs/hardhat-waffle';
import '@nomiclabs/hardhat-solpp';
import '@nomiclabs/hardhat-etherscan';
import 'hardhat-typechain';
import 'hardhat-contract-sizer';

const prodConfig = {
    // UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 1023,
    // PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: false,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_THRESHOLD
};

const testnetConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 1023,
    // PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: false,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_THRESHOLD
};

const testConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 5,
    PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: true,
    ZKSYNC_ADDRESS: process.env.CONTRACTS_CONTRACT_ADDR,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR,
    UPGRADE_GATEKEEPER_ADDRESS: process.env.CONTRACTS_UPGRADE_GATEKEEPER_ADDR,

    SECURITY_COUNCIL_MEMBERS_NUMBER: '3',
    // First 3 accounts obtained from `$ZKSYNC_HOME/etc/test_config/constant/test_mnemonic.json` mnemonic
    SECURITY_COUNCIL_MEMBERS:
        '0x36615Cf349d7F6344891B1e7CA7C72883F5dc049,0xa61464658AfeAf65CccaaFD3a512b69A83B77618,0x0D43eB5B8a47bA8900d84AA36656c92024e9772e',
    SECURITY_COUNCIL_THRESHOLD: '2'
};

const localConfig = Object.assign({}, prodConfig);
// @ts-ignore
localConfig.UPGRADE_NOTICE_PERIOD = 0;
localConfig.DUMMY_VERIFIER = process.env.CONTRACTS_TEST_DUMMY_VERIFIER === 'true';
// @ts-ignore
localConfig.NEW_ADDITIONAL_ZKSYNC_ADDRESS = process.env.CONTRACTS_ADDITIONAL_ZKSYNC_ADDR;

localConfig.SECURITY_COUNCIL_MEMBERS_NUMBER = process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER;
localConfig.SECURITY_COUNCIL_MEMBERS = process.env.MISC_SECURITY_COUNCIL_MEMBERS;
localConfig.SECURITY_COUNCIL_THRESHOLD = process.env.MISC_SECURITY_COUNCIL_THRESHOLD;

// @ts-ignore
localConfig.EASY_EXODUS = process.env.CONTRACTS_TEST_EASY_EXODUS === 'true';

const contractDefs = {
    rinkeby: testnetConfig,
    ropsten: testnetConfig,
    mainnet: prodConfig,
    test: testConfig,
    localhost: localConfig
};

export default {
    solidity: {
        version: '0.7.6',
        settings: {
            optimizer: {
                enabled: true,
                runs: 200
            },
            outputSelection: {
                '*': {
                    '*': ['storageLayout']
                }
            }
        }
    },
    contractSizer: {
        runOnCompile: false
    },
    paths: {
        sources: './contracts'
    },
    solpp: {
        defs: (() => {
            if (process.env.CONTRACT_TESTS) {
                return contractDefs.test;
            }
            return contractDefs[process.env.CHAIN_ETH_NETWORK];
        })()
    },
    networks: {
        env: {
            url: process.env.ETH_CLIENT_WEB3_URL?.split(',')[0]
        },
        hardhat: {
            allowUnlimitedContractSize: true,
            forking: {
                url: 'https://eth-mainnet.alchemyapi.io/v2/' + process.env.ALCHEMY_KEY,
                enabled: process.env.TEST_CONTRACTS_FORK === '1'
            }
        }
    },
    etherscan: {
        apiKey: process.env.MISC_ETHERSCAN_API_KEY
    }
};
