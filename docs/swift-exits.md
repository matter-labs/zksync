# Swift exits

## Goals

The main goal is to ensure the continuity of user flow. A typical use case is that the user has funds in the **committed** block and she wants to send them to an external address. But for this, she needs to wait for the verification of the block where she receives the funds, then the block where she performs the withdrawal operation. So swift exit allows requesting an immediate withdrawal of users' funds to any ethereum address, indicating his withdrawal transaction. Since the validators can guarantee to include a certain sequence of transactions in a block at some height, they can act as lenders and provide their funds for an immediate withdrawal on Layer 1.

## Structure

Main contracts:
- `SwiftExitsInternal` - contains the main internal landing methods that are responsible for the operation of the entire swift exits system, the processing of transaction, as well as for the storage of validators funds. This contract contains only internal methods common for `SwiftExitsEther` and `SwiftExitsErc20`. These methods are called from public methods of these contracts. The contract also contains non-implementing methods for receiving and sending funds, the implementation of which depends on whether the funds are *Ether* or *ERC-20* token. Methods are redefined and implemented in the respective contracts (`SwiftExitsEther` and `SwiftExitsErc20`).
- `SwiftExitsEther` - wraps an underlying `SwiftExitsInternal`, provides an interface for Ether transactions
- `SwiftExitsErc20` - wraps an underlying `SwiftExitsInternal`, provides an interface for ERC-20 tokens transactions
- `Rollup` - the main rollup contract, it processes withdraw operations

## Contract creation

The `SwiftExitsEther` and `SwiftExitsErc20` contract constructors include the creation of the `SwiftExitsInternal` contract.
To create a contract, you must specify:
- token contract address (`address(0)` in case of *Ether*)
- contract management address
- address of `Rollup` contract
- address of `BlsVerifier` contract
- address of the contract `owner`

When creating a contract, a token is checked in Governance contract.
Also, the last **verified** block will be received from `Rollup` contract.

The governor of Governance contract will need to specify the address of the created `SwiftExitsInternal` contract in the `Rollup` contract by calling the `addSwiftExits(tokenId, lendingAddress)` method.

## **Validators'** deposit

Validators can supply their *Ether* account directly by calling the `supply(to)` method of `SwiftExitsEther` contract with the value of the desired deposit in *Ether*.
To supply the account in *ERC-20* tokens, it is necessary to call the `supply(amount, to)` method on the corresponding `SwiftExitsErc20` contract.  Value of *Ether* is specified in the transactions' value option field.
The *to* field exists so that the user can supply to the account of another user if desired.

When these methods are executed, the internal method `supplyInternal(amount, to)` of `SwiftExitsInternal` contract, will be called. In this case, *ERC-20* tokens will be received on the contract through the `transferIn(amount)` method.

Next, corresponding records will be created in the mapping of the `SwiftExitsInternal` contract, reflecting the funds supplied to the user's account (by his ethereum address), as well as an increase in the funds available for lending.

## **Validators'** balance

The validator can find out the balance of his account from the validators' supplies mapping at any time, indicating his ethereum address as a key.

## **Validators'** funds withdrawal

The validator can call `requestWithdraw(amount, to)` method to withdraw his funds from his lending account (the validator MUST own this account and have enough funds on it).

In the case of a sufficient amount of non-borrowed funds on the contract, an immediate withdrawal of funds will occur.

Otherwise, a request for a deferred withdrawal of funds will be created.

### Immediate withdrawal

On `SwiftExitsInternal` contract, changes will occur: the change in the user's balance and the amount of funds available for the borrowing will decrease. If the user withdraws the full amount, then his account will be deleted.

### Deferred withdrawal

The available amount of funds will be withdrawn, for a gradual automatic withdrawal of the remaining, a `DefferedWithdrawOrder` will be created. Each time the available funds will be increased, all `DefferedWithdrawOrder`s will spend these funds until all of them are fulfilled.

## Swift exit request

The user can create and sign a request for a swift exit for his withdraw operation. Validators can verify the user's signature, as well as to sign this request themselves. When the required number of signatures is collected (2/3 of the total number of validators), they are aggregated and validated on the contract.

After that, on `Rollup` contract specified recipient balance will be reduced by the operation amount. So after validating the block with this operation, the total amount for it is equal to 0.

Depending on the amount of free funds on the `SwiftExitsInternal` contract, there will be created an Immediate exit request or Deffered exit request.

Thus, the user will borrow the free funds of validators. This debt will be repaid automatically when verification of the specified `Rollup` block containing the operation occurs. The full amount of this operation will be sent from `Rollup` contract to `SwiftExitsInternal` contract to cover costs and accrue validators fees.

### Immediate exit

If there is a sufficient amount of free funds, then an `immediateExit` metod will be called, otherwise a swift exit request (`SwiftExitOrder`) will be created.

`SwiftExitOrder` is tied to the corresponding block and contains information about the amount of funds and a payment recepient.

### Deffered exit

The validator can fulfill `SwiftExitOrder` request by its identifier in the request block through the `supplyOrder(blockNumber, orderId, sendingAmount, validator)` method. Thus, the validator will supply the balance of his account and free contract funds, and as a result, the `immediateExit` method will be called, after which the record of this request will be deleted.

### Transfering process

In the `transferOut(amount, receiver)` method, the specified funds will be transferred from the contract to the specified withdraw destination address.

Fees for each validator and contract holder will be calculated. The fee will be credited upon verification of the specified `Rollup` block.

Also, a deduction of a specified amount of funds from available funds of creditors will occur.

Fees are calculated in the `getCurrentInterestRates()` method, which validators and users can call to determine the need for themselves to participate in the lending process.

## Swift exit fulfillment

Upon verification of the next `Rollup` block, its borrow orders will be deleted, fees charged, and borrowed funds released. This operation is performed from the `Rollup` contract by calling the `newVerifiedBlock (blockNumber)` method.

### Swift exit repayment

Funds will be transferred to the SwiftExits contract through the call of `repayBorrow(amount)` method from the Rollup contract. This call is supposed to be made during verification of the corresponding `Rollup` block.

## Interest Rate calculations

Utilization ratio: 
`u = totalBorrowed / (totalSupply + totatBorrowed)`

Borrowing Interest Rate:
`BIR = MULTIPLIER * u + BASE_RATE`

Supply Interest Rate:
`SIR = BIR * u * (1 - SPREAD)`

Borrower fee:
`borrowerFee = bir * amount`

Validators fees:
`validatorsFees = borrowerFee * SIR`

Owner (Matter) fee:
`ownerFee = borrowerFee - validatorsFees`

Single validator fee:
`fee = validatorsFees * (validatorsSupplies[validatorId] / totalSupply)`

