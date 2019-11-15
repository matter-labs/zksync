# Swift exits

## Goals

The main goal is to ensure the continuity of user flow. A typical use case is that the user has tokens in the committed block and she wants to send them to an external address. But for this, she needs to wait for the verification of the block where she receives the tokens, then the block where she performs the withdrawal operation. So swift exit allows requesting an immediate withdrawal of users' tokens to any ethereum address, indicating his withdrawal operatioin. Since the validators can guarantee to include a certain sequence of operatioin in a block at some height, they can act as lenders and provide their tokens for an immediate withdrawal on Layer 1.

## Structure

Main components:
- `Governance` contract - contains the list of validators with their tokens, starts the process of SwiftExits operation. Validates aggregated signature, transfers/recieves tokens to/from `SwiftExits` contract (consummating fees to honest validators).
- `SwiftExits` contract - contains the list of **SwiftExits** requests. Validates their states, depending on corresponding withdraw operations on `Rollup` contract. Borrows token from `Compound` contract for validators supplied tokens and sends them to request recipient. Repays tokens recieved from `Rollup` contract to `Compound` contract and validators on `Governance` contract.
- `Rollup` contract - the main rollup contract, validates the signature of user-owner of the request. Freezes specified amount of tokens for recipient. On block verification defrosts this tokens. If request was successful - sends specifiend amount of tokens to `SwiftExits` contract to complete `SwiftExits` request.
- `Compound` contract - money market, allowes validators to borrow Ethereum assets for providing swift exit operation

## Swift exit algorithm

Simplified swift exit process without special cases of early verified block and punishment of validators:
![Algorithm scheme](https://i.imgur.com/6nHBY2f.png)

All process we can divide into 3 parts:
- Sidechain: user creates withdraw operation and corresponding swift exit request, supermajority of Rollup validators sign it
- Processing exit request on contracts: validating swift exit request, borrowing tokens for it, calculating fees, sending tokens to recipient
- Repaying borrow on contract: on blocks verify checks if corresponding withdraw operation exists, then any validator can send transaction to repay borrow to compound and consummate fees or punish validators depending on request status

### Sidechain

1. While creating an withdraw operation, the user can create and sign a request for a swift exit for this operation.
2. Since validators are responsible for including a user’s operation in a block, they can take responsibility for processing this request. Validators can verify the user's signature, as well as to sign this request themselves. When the required number of signatures is collected (2/3 of the total number of validators), validators send to `SwiftExits` contract following data:
   - Swift exit owner, recepient, token, amount, fee, block number, corresponding operation number
   - Owner signature
   - Aggregated signature
   - List of signers

### Processing exit request on contracts

1. Verification of validators aggregated signature on `Governance` contract and owner signature on `Rollup` 
2. If the specified block is not validated yet, there will be a check for enough free validators tokens on `Governance` contract (in Matter tokens)
3. Current rate and collateral factor for the token will be taken from `Compound` contract to calculate validators supply tokens amount
4. Swift exit data will be stored on `SwiftExits` contract, containing swift exit information
5. The required amount of validators tokens will be borrowed on `SwiftExits` contract to support borrow from compound
6. A borrow of tokens from `Compound` contract to `SwiftExits` contract for validators tokens will occurre
7. Tokens will be sent to the recipient from `SwiftExits` contract
8. Needed amount of tokens will be frozen on `Rollup` contract for request recipient

### Repaying borrow on contract

1. When the specified block is verified on `Rollup` contract there will be a check to find for swift exits their corresponding withdraw operations in block
2. The frozen funds will be defrosted on `Rollup` contract
3. If the block contains correct swift exit - its amount will be sent to `SwiftExits` contract
4. `SwiftExits` contract will repay a debt to `Compound` and will get validators tokens, fees will be sent to validators accounts on `Rollup` contract
5. If the verified block does not have the corresponding withdrawal operation, the punishment process will start. An intersection of the validators who signed the swift exit request and the validators who verified the block without corresponding withdraw operation will be found. The borrowed amount will be held from them in no return. The rest of the validators will receive their tokens

<!-- Full swift exit process without special cases of early verified block and punishment of validators. Contains complete inner contracts logic:
[Algorithm scheme](https://i.imgur.com/5UDLLGi.png) -->

<!-- ## Contract creation

The `SwiftExitsEther` and `SwiftExitsErc20` contract constructors include the creation of the `SwiftExitsInternal` contract.
To create a contract, you must specify:
- token contract address (`address(0)` in case of *Ether*)
- contract management address
- address of `Rollup` contract
- address of `BlsVerifier` contract
- address of the contract `owner`

When creating a contract, a token is checked in Governance contract.
Also, the last verified block will be received from `Rollup` contract.

The governor of Governance contract will need to specify the address of the created `SwiftExitsInternal` contract in the `Rollup` contract by calling the `addSwiftExits(tokenId, lendingAddress)` method.

## Validators' deposit

Validators can supply their *Ether* account directly by calling the `supply(to)` method of `SwiftExitsEther` contract with the value of the desired deposit in *Ether*.
To supply the account in *ERC-20* tokens, it is necessary to call the `supply(amount, to)` method on the corresponding `SwiftExitsErc20` contract.  Value of *Ether* is specified in the transactions' value option field.
The *to* field exists so that the user can supply to the account of another user if desired.

When these methods are executed, the internal method `supplyInternal(amount, to)` of `SwiftExitsInternal` contract, will be called. In this case, *ERC-20* tokens will be received on the contract through the `transferIn(amount)` method.

Next, corresponding records will be created in the mapping of the `SwiftExitsInternal` contract, reflecting the funds supplied to the user's account (by his ethereum address), as well as an increase in the funds available for lending.

## Validators' balance

The validator can find out the balance of his account from the validators' supplies mapping at any time, indicating his ethereum address as a key.

## Validators' funds withdrawal

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

Funds will be transferred to the SwiftExits contract through the call of `repayBorrow(amount)` method from the Rollup contract. This call is supposed to be made during verification of the corresponding `Rollup` block. -->

<!-- ## Interest Rate calculations

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
`fee = validatorsFees * (validatorsSupplies[validatorId] / totalSupply)` -->

