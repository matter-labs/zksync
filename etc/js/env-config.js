export default {
    'http://localhost': {
        API_SERVER: 'https://localhost:3001',
        WALLET_ADDRESS: 'http://localhost:3000',
        EXPLORER: 'http://localhost:7001',
        ETH_NETWORK: 'localhost',
        WS_API_ADDR: 'ws://localhost:3031',
        HTTP_RPC_API_ADDR: 'http://localhost:3030'
    },
    'https://explorer.dev.aggregation.rifcomputing.net': {
        API_SERVER: 'https://dev.aggregation.rifcomputing.net:3029',
        WALLET_ADDRESS: 'https://wallet.dev.aggregation.rifcomputing.net',
        EXPLORER: 'https://explorer.testnet.rsk.co',
        ETH_NETWORK: 'testnet',
        WS_API_ADDR: 'https://dev.aggregation.rifcomputing.net:3031',
        HTTP_RPC_API_ADDR: 'https://dev.aggregation.rifcomputing.net:3030'
    },
    'https://explorer.aggregation.rifcomputing.net': {
        API_SERVER: 'https://aggregation.rifcomputing.net:3029',
        WALLET_ADDRESS: 'https://wallet.aggregation.rifcomputing.net',
        EXPLORER: 'https://explorer.rsk.co',
        ETH_NETWORK: 'rsk_mainnet',
        WS_API_ADDR: 'https://aggregation.rifcomputing.net:3031',
        HTTP_RPC_API_ADDR: 'https://aggregation.rifcomputing.net:3030'
    }
}[`${location.protocol}//${location.hostname}`];
