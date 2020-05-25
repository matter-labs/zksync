import {ethers, Contract} from "ethers";
import * as fs from "fs";

const DETAILED_ERC20_ABI = require("openzeppelin-solidity/build/contracts/ERC20Detailed").abi;

const provider = ethers.getDefaultProvider("mainnet");

(async () => {
    // List of top 20 ERC20 tokens by daily transfer volume. (except VEN which is "old")
    const tokenAddresses = [
        "0x6b175474e89094c44da98b954eedeac495271d0f",
        "0xdac17f958d2ee523a2206206994597c13d831ec7",
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "0x514910771af9ca656af840dff83e8264ecf986ca",
        "0xB8c77482e45F1F44dE1745F52C74426C631bDD52",
        "0x8e870d67f660d95d5be530380d0ec0bd388289e1",
        "0xbd0793332e9fb844a52a205a233ef27a5b34b927",
        "0x75231f58b43240c9718dd58b4967c5114342a86c",
        "0xd0352a019e9ab9d757776f532377aaebd36fd541",
        "0x4fabb145d64652a948d72533023f6e7a623c7c53",
        "0x0000000000085d4780B73119b644AE5ecd22b376",
        "0x6f259637dcd74c767781e37bc6133cd6a68aa161",
        "0xd26114cd6EE289AccF82350c8d8487fedB8A0C07",
        "0x0ba45a8b5d5575935b8158a88c631e9f9c95a2e5",
        "0xe41d2489571d322189246dafa5ebde1f4699f498",
        "0x0d8775f648430679a709e98d2b0cb6250d2887ef",
        "0x1985365e9f78359a9B6AD760e32412f4a445E862",
        "0xb64ef51c888972c908cfacf59b47c1afbc0ab8ac",
        "0xb62132e35a6c13ee1ee0f84dc5d40bad8d815206",
        "0xb63b606ac810a52cca15e44bb630fd42d8d1d83d",
        "0xdd974d5c2e2928dea5f71b9825b8b646686bd200",
        "0x8971f9fd7196e5cee2c1032b50f656855af7dd26",
    ];

    // Sometimes data from chain is outdated
    function modifyOnchainData(token) {
        if (token.symbol === "MCO") {
            token.name = "MCO"; // Token name from chain is old
        } else if (token.symbol === "USDC") {
            token.name = "USD Coin"; // Weird name from chain
        }
        return token;
    }

    const result = [];
    for (const tokenAddress of tokenAddresses) {
        const erc20 = new Contract(tokenAddress, DETAILED_ERC20_ABI, provider);
        const address = tokenAddress;
        const decimals = await erc20.decimals();
        const symbol = await erc20.symbol();
        const name = await erc20.name();
        const token = modifyOnchainData({
            address,
            decimals,
            symbol,
            name,
        });
        console.log(`${token.address}, ${token.name}, ${token.symbol}, ${token.decimals}`);
        result.push(token);
    }
    fs.writeFileSync(`${process.env.ZKSYNC_HOME}/etc/tokens/mainnet.json`, JSON.stringify(result, null, 2));
})();
