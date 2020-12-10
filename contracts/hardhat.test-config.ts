import '@nomiclabs/hardhat-waffle';
import '@nomiclabs/hardhat-solpp';
import '@nomiclabs/hardhat-etherscan';
import 'hardhat-contract-sizer';
import { loadDefs } from './hardhat.utils';

export default {
    defaultNetwork: 'env',
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
        sources: './contracts',
        cache: './cache/test-contracts',
        artifacts: './artifacts/test-contracts'
    },
    solpp: {
        defs: loadDefs('test')
    },
    networks: {
        env: {
            url: `${process.env.WEB3_URL}`,
            allowUnlimitedContractSize: true
        }
    },
    etherscan: {
        apiKey: process.env.ETHERSCAN_API_KEY
    }
};
