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
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS,
    REGENESIS_MULTISIG_ADDRESS: process.env.MISC_REGENESIS_MULTISIG_ADDRESS,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_2_WEEKS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_2_WEEKS_THRESHOLD,
    SECURITY_COUNCIL_1_WEEK_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_1_WEEK_THRESHOLD,
    SECURITY_COUNCIL_3_DAYS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_3_DAYS_THRESHOLD
};
const testnetConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 1023,
    // PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: false,
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: process.env.MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS,
    REGENESIS_MULTISIG_ADDRESS: process.env.MISC_REGENESIS_MULTISIG_ADDRESS,

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_2_WEEKS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_2_WEEKS_THRESHOLD,
    SECURITY_COUNCIL_1_WEEK_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_1_WEEK_THRESHOLD,
    SECURITY_COUNCIL_3_DAYS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_3_DAYS_THRESHOLD
};

const testConfig = {
    UPGRADE_NOTICE_PERIOD: 0,
    MAX_AMOUNT_OF_REGISTERED_TOKENS: 5,
    PRIORITY_EXPIRATION: 101,
    DUMMY_VERIFIER: true,
    REGENESIS_MULTISIG_ADDRESS: '0xAA7113B9de498556dC76eDFEFc57681083c861C1',
    NEW_ADDITIONAL_ZKSYNC_ADDRESS: '0x7fbaD9d9C9a1204F45FA38CcbF732B0930F8B582',

    SECURITY_COUNCIL_MEMBERS_NUMBER: process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER,
    SECURITY_COUNCIL_MEMBERS: process.env.MISC_SECURITY_COUNCIL_MEMBERS,
    SECURITY_COUNCIL_2_WEEKS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_2_WEEKS_THRESHOLD,
    SECURITY_COUNCIL_1_WEEK_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_1_WEEK_THRESHOLD,
    SECURITY_COUNCIL_3_DAYS_THRESHOLD: process.env.MISC_SECURITY_COUNCIL_3_DAYS_THRESHOLD
};

const localConfig = Object.assign({}, prodConfig);
// @ts-ignore
localConfig.UPGRADE_NOTICE_PERIOD = 0;
localConfig.DUMMY_VERIFIER = process.env.CONTRACTS_TEST_DUMMY_VERIFIER === 'true';
// @ts-ignore
localConfig.REGENESIS_MULTISIG_ADDRESS = process.env.MISC_REGENESIS_MULTISIG_ADDRESS;
// @ts-ignore
localConfig.NEW_ADDITIONAL_ZKSYNC_ADDRESS = process.env.MISC_NEW_ADDITIONAL_ZKSYNC_ADDRESS;

localConfig.SECURITY_COUNCIL_MEMBERS_NUMBER = process.env.MISC_SECURITY_COUNCIL_MEMBERS_NUMBER;
localConfig.SECURITY_COUNCIL_MEMBERS = process.env.MISC_SECURITY_COUNCIL_MEMBERS;
localConfig.SECURITY_COUNCIL_2_WEEKS_THRESHOLD = process.env.MISC_SECURITY_COUNCIL_2_WEEKS_THRESHOLD;
localConfig.SECURITY_COUNCIL_1_WEEK_THRESHOLD = process.env.MISC_SECURITY_COUNCIL_1_WEEK_THRESHOLD;
localConfig.SECURITY_COUNCIL_3_DAYS_THRESHOLD = process.env.MISC_SECURITY_COUNCIL_3_DAYS_THRESHOLD;

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
            allowUnlimitedContractSize: true
        }
    },
    etherscan: {
        apiKey: process.env.MISC_ETHERSCAN_API_KEY
    }
};
