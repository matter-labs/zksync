import '@nomiclabs/hardhat-waffle';
import '@nomiclabs/hardhat-solpp';
import 'hardhat-typechain';
import 'hardhat-contract-sizer';

export default {
    solidity: {
        version: '0.7.3',
        settings: {
            optimizer: {
                enabled: true,
                runs: 200
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
        defs: {
            UPGRADE_NOTICE_PERIOD: 0,
            MAX_AMOUNT_OF_REGISTERED_TOKENS: 5,
            PRIORITY_EXPIRATION: 101,
            DUMMY_VERIFIER: true,
        }
    }
};
