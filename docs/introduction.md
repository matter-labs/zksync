# Introduction

**zksync 0.1** allows user to make cheap transactions of ETH and selected subset of ERC20 tokens using zero knowledge rollup.

Cheap transaction are executed in the sidechain network.

In order to receive and use funds in the sidechain network user have to have private/public key pair and address which is derived from this public key. Keys are derived from users ethereum wallet.

## Deposit

In order to move funds into sidechain user have to make eth transaction to the sidechain contract, this creates deposit request. When its completed, funds are available in the sidechain network.

## Transfer

To send ETH/ERC20 token in the sidechain user have to create sidechain transaction and send it to the sidechain operator. Sender should known recipient sidechain address.

## Withdraw

In order to move funds from sidechain user have to:

1\) Create sidechain transaction. This creates withdraw request.  
2\) After withdraw request is confirmed on the ethereum network, user can withdraw funds from sidechain contract.

