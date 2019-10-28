# Lending

## Goals

The main goal is to ensure the continuity of user flow. A typical use case is that the user has funds in the **committed** block and she wants to send them to an external address. But for this, she needs to wait for the verification of the block where she receives the funds, then the block where she performs the withdrawal operation. So lending allows requesting an immediate withdrawal of users' funds to any ethereum address, indicating the **committed** block withdrawal transaction. In this case, the verifiers, acting as lenders, trust the system and the fact that the block will be **verified**, and provide their funds for conducting the withdrawal transaction. This operation is conditionally called lending and for it, lenders will receive a fee, depending on the current state of the supplied and borrowed balances.

## Structure

Main contracts:
- `LendingToken`
- `LendingEther`
- `LendingErc20`
- `Franklin`

## `LendingToken` contract

This contract contains only internal methods common to `LendingEther` and `LendingErc20`. These methods are called from public methods of these contracts.
The contract also contains non-implementing methods for receiving and sending funds, the implementation of which depends on whether the funds are *Ether* or *ERC-20* token. Methods are redefined and implemented in the respective contracts (`LendingEther` and `LendingErc20`).

## Contract creation

The `LendingEther` and `LendingErc20` contract constructors include the creation of the `LendingToken` contract.
To create a contract, you must specify:
- token contract address (`address(0)` in case of *Ether*)
- contract management address
- address of `Franklin` contract
- address of `Verifier` contract
- address of the contract `owner`

When creating a contract, a token is checked in Governance contract.
Also, the last **verified** block will be received from `Franklin` contract.

The governor of Governance contract will need to specify the address of the created lending contract in the `Franklin` contract by calling the `addLending(tokenId, lendingAddress)` method.

## **Lenders'** deposit

Lenders can supply their *Ether* account directly by calling the `supply(to)` method of `LendingEther` contract with the value of the desired deposit in *Ether*.
To supply the account in *ERC-20* tokens, it is necessary to call the `supply(amount, to)` method on the corresponding `LendingErc20` contract.  Value of *Ether* is specified in the transactions' value option field.
The *to* field exists so that the user can supply to the account of another user if desired.

When these methods are executed, the internal method `supplyInternal(amount, to)` of `LendingToken` contract, will be called. In this case, *ERC-20* tokens will be received on the contract through the `transferIn(amount)` method.

Next, corresponding records will be created in the mapping of the `LendingToken` contract, reflecting the funds supplied to the user's account (by his ethereum address), as well as an increase in the funds available for lending.

## **Lenders'** balance

The lender can find out the balance of his account from the lenders' supplies mapping at any time, indicating his ethereum address as a key.

## **Lenders'** funds withdrawal

The lender can call `requestWithdraw(amount, to)` method to withdraw his funds from his lending account (the lender MUST own this account and have enough funds on it).

In the case of a sufficient amount of non-borrowed funds on the contract, an immediate withdrawal of funds will occur.

Otherwise, a request for a deferred withdrawal of funds will be created.

### Immediate withdrawal

On `LendingToken` contract, changes will occur: the change in the user's balance and the amount of funds available for the borrowing will decrease. If the user withdraws the full amount, then his account will be deleted.

### Deferred withdrawal

The available amount of funds will be withdrawn, for a gradual automatic withdrawal of the remaining, a `DefferedWithdrawOrder` will be created. Each time the available funds will be increased, all `DefferedWithdrawOrder`s will spend these funds until all of them are fulfilled.

## Borrow request

If there is a **committed** block containing the withdrawal transaction with the address of this lending contract as the recipient of the withdrawal, the user can send a request for the `LendingEther` and `LendingErc20` contracts to immediately send the indicated funds to an ethereum address (he MUST be this transactions' "owner")

Thus, he will borrow the free funds of creditors. This debt will be repaid automatically when verification of the specified `Franklin` block containing the transaction occurs.

The user must call the `requestBorrow(onchainOpNumber, amount, borrower, receiver, signature)` method of the `LendingEther` and` LendingErc20` contracts.
Next, the `requestBorrowInternal` method of the contract `LendingToken` will be called, on which the necessary checks will occur:
- the block must be unverified
- the borrowing amount must be positive
- signature verification must be successful
- verification of the borrow request in the specified Franklin block must be successful (on the `Franklin` contract MUST be the corresponding request, that must contain the same number, token amount, borrower).

### Immediate borrow

If there is a sufficient amount of free funds, then an immediate borrow will occur, otherwise a borrow request (`BorrowOrder`) will be created.

`BorrowOrder` is tied to the corresponding block and contains information about the amount of funds and a payment recepient.

### Deffered borrow

The lender can fulfill this request by its identifier in the request block through the `fulfillOrder(blockNumber, orderId, sendingAmount, lender)` method. Thus, the lender will supply the balance of his account and free contract funds, and as a result, the `immediateBorrow` method will be called, after which the record of this request will be deleted.

### Borrow process

In the `transferOut(amount, receiver)` method, the specified funds will be transferred from the contract to the specified withdraw destination address.

Fees for each lender and contract holder will be calculated. The fee will be credited upon verification of the specified Franklin block.

Also, a deduction of a specified amount of funds from available funds of creditors will occur.

Fees are calculated in the `getCurrentInterestRates()` method, which lenders and users can call to determine the need for themselves to participate in the lending process.

## Borrow fulfillment

Upon verification of the next `Franklin` block, its borrow orders will be deleted, fees charged, and borrowed funds released. This operation is performed from the `Franklin` contract by calling the `newVerifiedBlock (blockNumber)` method.

### Borrow repayment

Funds will be transferred to the Lending contract through the call of `repayBorrow(amount)` method from the Franklin contract. This call is supposed to be made during verification of the corresponding `Franklin` block.

## Interest Rate calculations

Utilization ratio: 
`u = totalBorrowed / (totalSupply + totatBorrowed)`

Borrowing Interest Rate:
`BIR = MULTIPLIER * u + BASE_RATE`

Supply Interest Rate:
`SIR = BIR * u * (1 - SPREAD)`

Borrower fee:
`borrowerFee = bir * amount`

Lenders fees:
`lendersFees = borrowerFee * SIR`

Owner (Matter) fee:
`ownerFee = borrowerFee - lendersFees`

Single lender fee:
`fee = lendersFees * (lendersSupplies[lenderId] / totalSupply)`

