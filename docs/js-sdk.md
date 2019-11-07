---
description: 'https://www.npmjs.com/package/zksync'
---

# JS SDK

#SidechainProvider
Handles requests to the sidechain node.

## static async newWebsocketProvider(address: string): SidechainProvider

## static async newHTTPProvider(address: string): SidechainProvider

## sideChainInfo

ontract_address - address of the sidechain main contract
tokens - list of supported tokens

# Wallet
Used to interact with sidechain.
Each sidechain wallet is derrived from ETH wallet.
Actions:
deposit - move funds from ETH wallet to sidechain (slow, expensive)
transfer - move funds between sidechain accounts (fast, cheap)
withdrawFromSiechain - move funds from sidechain to ETH contract
withdrawFromContract - move funds from ETH contract to ETH wallet.

All numerical amounts are "ethers.utils.BigNumberish"

## async fromEthWallet( ethSigner, provider): Wallet
## Creates sidechain wallet (key pair) from ETH wallet.

## address
Sidechain address of the wallet.
0x[hex encoded 20 bytes]

## async deposit(token, amount, max_eth_fee): DepositHandle
token - token to be deposited.

amount - amount of tokens

max_eth_fee - max ETH fee for this transaction. (! fees for deposit are payed in ETH)

## async transfer(to, token, amount, fee, [nonce]): TxHandle
to - sidechain address of recepient

token - token to be transferred

amount - amount of tokens to transfer

fee - amount of tokens to pay as a fee

nonce - sidechain nonce for this transaction.

## async withdrawFromSidechain(token, amount, fee, [nonce]): TxHandle
token - token to be withdrawn

amount - amount of tokens for withdraw

fee - tokens to pay as a fee

nonce - sidechain nonce for this transaction

## async withdrawFromContract(token, amount): ethers.ContractTransaction
## async getSidechainState(): SidechainAccountState
## async getETHBalances(): ETHAccountState
