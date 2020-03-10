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
    "https://demo.zksync.dev/": {
        API_SERVER: "https://testnet.zksync.dev",
        ETH_NETWORK: "rinkeby",
        WS_API_ADDR: "wss://testnet.zksync.dev/jsrpc-ws",
        HTTP_RPC_API_ADDR: "https://testnet.zksync.dev/jsrpc",
    },
}[`${location.protocol}//${location.hostname}`];
