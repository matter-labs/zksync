export default {
    "http://localhost": {
        API_SERVER: "http://localhost:3001",
        ETH_NETWORK: "localhost",
        WS_API_ADDR: "ws://localhost:3031",
        HTTP_RPC_API_ADDR: "http://localhost:3030",
    },
    "https://stage.zksync.dev": {
       API_SERVER: "https://stage-api.zksync.dev",
       ETH_NETWORK: "rinkeby",
       WS_API_ADDR: "wss://stage-api.zksync.dev/jsrpc-ws",
       HTTP_RPC_API_ADDR: "https://stage-api.zksync.dev/jsrpc",
    },
    "https://rinkeby.zkscan.io": {
        API_SERVER: "https://rinkeby-api.zksync.io",
        ETH_NETWORK: "rinkeby",
        WS_API_ADDR: "wss://rinkeby-api.zksync.io/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://rinkeby-api.zksync.io/jsrpc",
    },
    "https://ropsten.zkscan.io": {
        API_SERVER: "https://ropsten-api.zksync.io",
        ETH_NETWORK: "ropsten",
        WS_API_ADDR: "wss://ropsten-api.zksync.io/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://ropsten-api.zksync.io/jsrpc",
    },
    "https://zkscan.io": {
        API_SERVER: "https://api.zksync.io",
        ETH_NETWORK: "mainnet",
        WS_API_ADDR: "wss://api.zksync.io/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://api.zksync.io/jsrpc",
    },
    "https://dev.zksync.dev": {
        API_SERVER: "https://dev-api.zksync.dev",
        ETH_NETWORK: "rinkeby",
        WS_API_ADDR: "wss://dev-api.zksync.dev/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://dev-api.zksync.dev/jsrpc",
    },
    "https://breaking.zksync.dev": {
        API_SERVER: "https://breaking-api.zksync.dev",
        ETH_NETWORK: "rinkeby",
        WS_API_ADDR: "wss://breaking-api.zksync.dev/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://breaking-api.zksync.dev/jsrpc",
    },
}[`${location.protocol}//${location.hostname}`];
