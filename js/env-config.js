export default {
    "http://localhost": {
        API_SERVER: "http://localhost:3000",
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
        API_SERVER: "https://rinkeby-api.zksync.dev",
        ETH_NETWORK: "rinkeby",
        WS_API_ADDR: "wss://rinkeby-api.zksync.dev/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://rinkeby-api.zksync.dev/jsrpc",
    },
    "https://ropsten.zkscan.io": {
        API_SERVER: "https://ropsten-api.zksync.dev",
        ETH_NETWORK: "ropsten",
        WS_API_ADDR: "wss://ropsten-api.zksync.dev/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://ropsten-api.zksync.dev/jsrpc",
    },
}[`${location.protocol}//${location.hostname}`];
