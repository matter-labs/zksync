## Priority Queue

![PriorityQueue Contract architecture](https://i.imgur.com/fSapuew.png)

This queue will be implemented in separate contract to ensure that _priority_ operations like _deposits_ and _full_exits_ will be processed in a timely manner and will be included in one of Franklin's blocks (a situation that leads to the need to reverse blocks will not happen), and also, in the case of _full_exit_ transactions, the user can always withdraw funds (censorship-resistance). Its' functionality is divided into 2 parts: **Requests Queue** and **Exodus Mode**.

### Requests Queue

#### *PriorityRequest* structure:

Only Franklin account is allowed to be transactor to **Priority Queue**.
So the user must send her request to **Franklin** main contract and then the corresponding request will be sent to **Priority Queue**.

*PriorityRequest* must be sent with pubData in bytes array as payload.

Example of pubData:
- `opType` - operation type (REQUIRED FIELD)
- `franklinAddress` - Franklin address
- `ethAddress` - Ethereum address
- `token` - selected token
- `amount` - the amount of selected token
- `signature` created by the _user_

When *PriorityRequest* comes to **PriorityQueue** the check for the presence of that request in mappings occurs. If the check succeeds *PriorityRequest* will get an additional field:
- `expirationBlock` - the Ethereum block until which this request must be satisfied (transaction is included in the Franklin block). `expirationBlock` is calculated as follows:
`expirationBlock = block.number + 250` - about 1 hour for the transaction to expire, `block.number` - current Ethereum block number.

Requests expiration blocks are saved in mapping in the order they are received to enable the possibility of tracking their status.

#### **Requests Queue** interface:

- `function addRequest(pubData)`: create new *PriorityRequest*
- `event NewRequest(pubData, expirationBlock)`: event emitted by addRequest function

#### Deposits example

The user must send a deposit *PriorityRequest* to the **Franlin** contract and corresponding request will be sent from **Franklin** to **Priority Queue** that implements **Requests Queue** in order to move funds from her locked **root-chain balance** on **Franklin contract** into **Franklin chain**. This will lead to including a signed **circuit operation** `deposit` in the Franklin block.

When **Operator** produces a block which contains a **circuit operation** `deposit`, the requested amount is removed from the locked **root-chain balance** to selected *Franklin account balance*.

Note: although any validator can try to include deposited funds from any user into their block without permission, this situation does not require special treatment. It constitutes a general DOS attack on the network (because the validator won't be able to prove the authorization) and thus will be ruled out in the generic fashion.

#### Full exits example

If the user's exit transaction is not inserted into any block, then to return funds, she can send a request to the **Franlin** contract and corresponding request will be sent from **Franklin** to **Priority Queue** that implements **Requests Queue**. This will lead to including a signed **circuit operation** `full_exit` in the Franklin block.

When **Operator** produces a block which contains a **circuit operation** `full_exit`, all requested tokens amount is removed from the **Franklin account balance** to **root-chain balance**.

### **Operator** responsibility

**Operators** MUST subscribe for `NewRequest` events in the RIGHT order to include priority transactions in some upcoming blocks.
The need for _Operators_ to include these transactions in blocks as soon as possible is dedicated by the increasing probability of entering the **Exodus Mode** (described below).

A certain value of the selected token will be withdrawn from the _user's_ account, as payment for the _operator’s_ work to include these transactions in the block. One transaction fee is calculated as follows:
`fee = 3 * gas * mediumFee`, where
- `gas` - the gas cost of all related operations for the exit
- `mediumFee` - current average fee in the network.

The value of the field `amount` from _deposit_ transaction included in the **Franklin block** can be changed to zero if there will be not enough funds on **root-chain balance**.

The _full_exit_ transaction included in the **Franklin block** will contain a `fullAmount` field - the value of tokens that will be transferred from the user account to his root-chain balance. If `fullAmount` field is equal to zero - there is no tokens on Franklin chain user balance.

### **Franklin** contract responsibility

Every upcoming block will be scanned for existing priority transactions. Then priority transactions count will be sent to **Priority Queue** function `executeRequests(count)` and if corresponding `expirationBlocks` mapping values will be deleted.

### **Exodus Mode**

In the **Exodus mode**, the contract freezes all block processing, and all users must exit.

Every user will be able to submit a SNARK proof that she owns funds in the latest verified state of Franklin by calling `exit()` function. If the proof verification succeeds, the entire user amount will be accrued to her **onchain balance**, and a flag will be set in order to prevent double exits of the same token balance.

#### Triggering **Exodus Mode**
If the **Priority Queue** is being processed too slow, it will trigger the **Exodus mode** in **Franklin** contract. This moment is determined by the first (oldest) `expirationBlock` mapping value on the **Priority Queue** contract. The trigger occurs when **Franklin** checks the need to enter it via `isExodusActivated(currentEthereumBlock)` function on **Priority Queue** contract. If `currentEthereumBlockNumber >= oldestExpirationBlockNumber` the **Exodus Mode** will be entered.