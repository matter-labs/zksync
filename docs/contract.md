# ZKSync ETH contract documentation 合约文档

![ZKSync Contract Onchain Operations](https://i.imgur.com/Y3taY1y.png)

## Deployment 部署

The contract must be deployed specifying the initial ("genesis") state root hash, appointing the **network governor** (see the "Governance" section), and linking the exit queue (see the "Cenosorship resistance" section).

合约必须在部署时指定初始（“创世”）状态根哈希，任命**网络治理者**（参见“治理”部分）并链接退出队列（参见“防止审查”部分）。

## Governance 治理

Governance of the network will be excerised from a separate contract registered in the ZKSync contract as `networkGovernor`. It has the power to:
网络的治理将从在ZKSync合约中注册的单独合约中进行，称为`networkGovernor`。它有以下权限：
- Change the set of validators.
- 更改验证器集合。
- Add new tokens (tokens can not be removed after being added).
- 添加新的代币（添加后无法删除）。
- Initiate migration to a new contract (see the "Migration" section).
- 启动迁移至新合约（参见“迁移”部分）。

## Cenosorship resistance 抗审查

To enforece censorship-resistance and enable guaranteed retrievability of the funds, ZKSync employs the mechanisms of **Priority queue** (soft enforcement) and **Exodus mode** (hard enforcement).

为了执行防审查和使资金的可靠可检索性，ZKSync使用**Priority queue**（软强制执行）和 **Exodus mode**（硬强制执行）的机制。

## Deposits 存款

To make deposit, a user can:
要进行存款，用户可以

- Either send ETH to smart contract (will be handled by the default function),
- 向智能合约发送 ETH（将由默认函数处理），
- or call `depositERC20()` function to perform transferFrom for a registered ERC20 token. Note: the user must have previously called approve() on the ERC20 token contract in order to authorize ZKSync contract to perform this operation.
- 或调用`depositERC20()`函数以执行已注册ERC20代币的transferFrom操作。注意：用户必须事先调用approve()函数在ERC20代币合约上，以授权ZKSync合约执行此操作。

This deposit creates **deposit priority request** that is placed in corresponding priority requests mapping and also emits **NewPriorityRequest(opType, pubData, expirationBlock)** event to notify validators that they must include this request to upcoming blocks. Complete **PriorityQueue** logic that handles **priority requests** is described in **Priority Requests** section.

该存款创建了一个 **deposit priority request**，它被放置在相应的优先请求映射中，并且还发出 **NewPriorityRequest(opType, pubData, expirationBlock)** 事件，以通知验证者他们必须将此请求包含在即将到来的块中。完整的 **PriorityQueue** 逻辑，处理  **priority requests**，在 **Priority Requests** 部分中描述。

When a validator commits a block which contains **circuit operations** `deposit`, **deposit onchain operation** for this deposit is created to verify compliance with priority queue requests. If it succeeds than their count will be added to **priority requests** count for this block. If the block is not verified, **deposit onchain operations** and **deposit priority request** are simply discarded.

当验证者提交包含 **circuit operations** `deposit` 的块时，将创建 **deposit onchain operation** 来验证优先级队列请求的合规性。如果成功，则将它们的计数添加到此块的 **priority requests** 计数中。如果块未通过验证，则 **deposit onchain operations** 和 **deposit priority request** 将被简单地丢弃。

If the block is reverted **deposit onchain operations** are simply discarded.
如果块被回滚，则 **deposit onchain operations** 将被简单地丢弃。

If ZKSync contract has entered Exodus mode and the block is unverified, the funds held by this blocks' **Deposit priority requests** are accrued to the owners' **root-chain balances** to make them possible to withdraw. This **withdraw onchain operations** and **full exit priority requests** are simply discarded.

如果 ZKSync 合约已进入 Exodus mode 并且该块未通过验证，则由该块的 **Deposit priority requests** 持有的资金将累积到其所有者的 **root-chain balances**，以使它们能够进行提取。这个 **withdraw onchain operations** 和 **full exit priority requests** 将被简单地丢弃。

## Withdrawals 提现

### Partial withdrawal 部分提现

It is a standard withdrawal operation. When a block with `partial_exit` **circuit operation** is committed, **withdraw onchain operation** for this withdrawal is created. If the block is verified, funds from the **withdrawal onchain operation** are acrued to the users' **root-chain balances**.

部分提现是一项标准操作。当提交了一个包含 `partial_exit` **电路操作** 的区块时，会创建一个对应的 **提现 onchain 操作**。如果该区块被验证，则从该 **withdrawal onchain operation** 中的资金会累积到用户的 **root-chain balances** 中。

If the block is reverted, this **withdraw onchain operations** are simply discarded.

如果区块被回滚，则该 **withdraw onchain operations** 将被简单地丢弃。

A user can withdraw funds from the **root-chain balance** at any time by calling a `withdrawETH()` or `withdrawERC20()` function.

用户可以通过调用 `withdrawETH()` 或 `withdrawERC20()` 函数随时从其 **root-chain balance** 中提取资金

### Full exit 全部提现

User can request this expensive operation to withdraw funds if he thinks that his transactions are censored by validators.

如果用户认为其交易受到验证者的审查，可以请求此项昂贵的操作以提取资金。

The user must send a transaction to **ZKSync** contract function `registerFullExit()`. This function creates **full exit priority request** that is placed in corresponding priority requests mapping and also emits **NewPriorityRequest(serialId, opType, pubData, expirationBlock)** event to notify validators that they must include this request to upcoming blocks. Complete **PriorityQueue** logic that handles **priority requests** is described in **Priority Requests** section.

用户必须向 **ZKSync** 合约函数 `registerFullExit()` 发送一笔交易。该函数将创建一个 **full exit priority request**，并将其放置在相应的优先请求映射中，还会发出 **NewPriorityRequest(serialId, opType, pubData, expirationBlock)** 事件以通知验证者他们必须在即将到来的区块中包含该请求。完整的 **PriorityQueue** 逻辑，用于处理 **priority requests**，在 **Priority Requests** 部分中有描述。

When a validator commits a block which contains a **circuit operation** `full_exit`, the corresponding **withdraw onchain operation** for this withdrawal is created to verify compliance with priority queue requests. If it succeeds than their count will be added to **priority requests** count for this block. If the block is verified, funds from the **withdrawal onchain operation** are accrued to the users' **root-chain balances** and **withdraw onchain operations** and **full exit priority requests** are simply discarded.

当验证者提交一个包含 **电路操作** `full_exit` 的区块时，将创建相应的 **withdraw onchain operation**，以验证其与优先级队列请求的符合性。如果验证成功，则其计数将添加到该块的 **priority requests** 计数中。如果该区块被验证，则从该 **withdrawal onchain operation** 中的资金会累积到用户的**root-chain balances** 中，并且 **withdraw onchain operations** 和 **full exit priority requests** 将被简单地丢弃。

If the block is reverted, this **withdraw onchain operations** are simply discarded.

如果区块被回滚，则该 **withdraw onchain operations** 将被简单地丢弃。

If ZKSync contract has entered Exodus mode and the block is unverified, this **withdraw onchain operations** and **full exit priority requests** are simply discarded.

如果 ZKSync 合约进入 Exodus 模式且该区块未被验证，则该 **withdraw onchain operations** 和 **full exit priority requests** 将被简单地丢弃。

## Block committment 区块提交

Only a sender from the validator set can commit a block.

只有来自验证器集的发送者才能提交区块。

The number of committed but unverified blocks is limited by `MAX_UNVERIFIED_BLOCKS` constant in order to make sure that
gas is always sufficient to revert all committed blocks if the verification has not happened in time.

已提交但未验证的区块数量由 `MAX_UNVERIFIED_BLOCKS` 常量限制，以确保如果验证未及时完成，则永远有足够的gas用于撤销所有已提交的区块。

## Block verification 区块验证

Anybody can perform verification for the committed block.

任何人都可以对提交的区块进行验证。

## Reverting expired blocks 回退过期区块

If the first committed block was not verified within `EXPECT_VERIFICATION_IN` ETH blocks, all unverified blocks will be reverted and the funds held by **onchain operations** and **priority requests** will be released and stored on **root-chain balances**.

如果第一个提交的区块未在 `EXPECT_VERIFICATION_IN` 个ETH块内得到验证，则所有未验证的区块将被撤销，并且由 **onchain operations** 和 **priority requests** 持有的资金将被释放并存储在 **root-chain balances** 中。

## Priority queue 优先级队列

This queue will be implemented in separate contract to ensure that priority operations like `deposit` and `full_exit` will be processed in a timely manner and will be included in one of ZKSync's blocks (a situation that leads to the need to reverse blocks will not happen), and also, in the case of `full_exit` transactions, the user can always withdraw funds (censorship-resistance). Its' functionality is divided into 2 parts: **Requests Queue** and **Exodus Mode**.

这个队列将在单独的合约中实现，以确保优先级操作，如 `deposit` 和 `full_exit`，能够及时处理并被包含在 ZKSync 的区块中（不会出现需要撤销区块的情况），并且在 `full_exit` 交易的情况下，用户可以随时提取资金（具有防止审查的特性）。其功能分为两部分： **Requests Queue** 和 **Exodus Mode**。

**NewPriorityRequest** event is emitted when a user send according transaction to ZKSync contract. Also some info about it will be stored in the mapping (operation type and expiration block) strictly in the order of arrival. **NewPriorityRequest** event structure:

当用户向 ZKSync 合约发送相应的交易时，将触发 **NewPriorityRequest** 事件。此外，一些关于该事件的信息将严格按照到达顺序存储在映射中（操作类型和到期块）。**NewPriorityRequest** 事件结构如下：

- `serialId` - serial id of this priority request
- `serialId` - 此优先请求的序列号
- `opType` - operation type
- `opType` - 操作类型
- `pubData` - request data
- `pubData` - 请求数据
- `expirationBlock` - the number of Ethereum block when request becomes expired `expirationBlock` is calculated as   follows: `expirationBlock = block.number + 250` - about 1 hour for the transaction to expire, `block.number` - current  Ethereum block number.
- `expirationBlock` - 请求将过期的以太坊块的数量。计算公式如下：`expirationBlock` = `block.number` + `250`，大约需要1小时时间使交易过期，其中 `block.number` 是当前以太坊块的编号。

When corresponding transactions are found in the commited block, their count must be recorded. If the block is verified, this count of the satisfied **priority requests** is removed from mapping.

当在提交的块中找到相应的交易时，必须记录它们的数量。如果该块已经被验证，则从映射中删除这些满足的 **priority requests** 的计数。

If the block is reverted via Exodus Mode, the funds held by **Deposit priority requests** from this block are accrued to the owners' **root-chain balances** to make them possible to withdraw. And this **Deposit priority requests** will be removed from mapping.

如果通过 Exodus 模式撤销该块，则来自该块的 **Deposit priority requests** 中持有的资金将计入所有者的 **root-chain balances** 中，以便可以提取。并且这些 **Deposit priority requests** 将从映射中删除。

### Fees for Priority Requests 优先请求的费用

In order to send priority request, the _user_ MUST pay some extra fee. That fee will be subtracted from the amount of Ether that the user sent to Deposit or Full Exit funcitons. That fee will be the payment for the _validator’s_ work to include these transactions in the block. One transaction fee is calculated as follows: `fee = FEE_COEFF * (BASE_GAS + gasleft) * gasprice`, where

为了发送优先请求，用户 __必须__ 支付一些额外的费用。该费用将从用户发送到 Deposit 或 Full Exit 函数的以太币中扣除。该费用将支付 验证人 将这些交易包含在区块中的费用。一个交易的费用计算如下：`fee = FEE_COEFF * (BASE_GAS + gasleft) * gasprice`，其中

- `FEE_COEFF` - fee coefficient for priority request transaction
- `FEE_COEFF` - 优先请求交易的费用系数
- `BASE_GAS` - base gas cost for transaction (usually 21000)
- `BASE_GAS` - 交易的基本燃料成本（通常为21000）
- `gasleft` - remaining gas for transaction code execution
- `gasleft` - 交易代码执行剩余的燃料
- `gasprice` - gas price of the transaction
- `gasprice` - 交易的燃料价格

If the user sends more Ether than necessary, the difference will be returned to him.

如果用户发送的以太币超过所需金额，则差额将退还给用户。

### **Validators'** responsibility 验证者的责任

**Validators** MUST subscribe for `NewPriorityRequest` events in the RIGHT order to include priority transactions in some upcoming blocks. The need for _Validators_ to include these transactions in blocks as soon as possible is dedicated by the increasing probability of entering the **Exodus Mode** (described below).

**验证者** 必须以正确的顺序订阅 `NewPriorityRequest` 事件，以在即将到来的块中包括优先交易。将这些交易尽快包括在区块中的需求，由进入下面所述的 **Exodus** 模式 的概率逐渐增加。

### Exodus mode Exodus 模式

If the **Requests Queue** is being processed too slow, it will trigger the **Exodus mode** in **ZKSync** contract. This moment is determined by the first (oldest) **priority request** with oldest `expirationBlock` value . If `current ethereum block number >= oldest expiration block number` the **Exodus Mode** will be entered.

如果 **Requests Queue** 的处理速度过慢，将触发 ZKSync 合约中的 **Exodus 模式** 。这个时刻是由具有最旧 `expirationBlock` 值的第一个（最老的）优先请求决定的。如果 `current ethereum block number >= oldest expiration block number`，则将进入 **Exodus 模式**。

In the **Exodus mode**, the contract freezes all block processing, and all users must exit. All existing block commitments will be reverted.

在 **Exodus 模式** 中，合约将冻结所有区块处理，并且所有用户都必须退出。所有现有的区块承诺将被回滚。

Every user will be able to submit a SNARK proof that she owns funds in the latest verified state of ZKSync by calling `exit()` function. If the proof verification succeeds, the entire user amount will be accrued to her **onchain balance**, and a flag will be set in order to prevent double exits of the same token balance.

每个用户将能够通过调用 `exit()` 函数提交一个 SNARK 证明，证明她拥有在 ZKSync 的最新验证状态中的资金。如果验证成功，整个用户金额将累计到她的 **onchain balance** 中，并且将设置一个标志以防止相同令牌余额的双重退出。

## Migration 迁移

(to be implemented later  待实施)

ZKSync shall always have a strict opt-in policy: we guarantee that user funds are retrievable forever under the conditions a user has opted in when depositing funds, no matter what. A migration to a newer version of the contract shall be easy and cheap, but MUST require a separate opt-in or allow the user to exit.

ZKSync 必须始终具有严格的选择加入政策：我们保证用户资金在用户存入资金时选择的条件下，无论如何永远可检索。升级到合约的新版本应该简单且廉价，但必须要求单独的选择加入或允许用户退出。

The update mechanism shall follow this workflow:

更新机制应遵循以下工作流程：

- The **network governor** can schedule an update, specifying a target contract and an ETH block deadline.
- **网络管理者** 可以安排更新，指定目标合约和 ETH 块截止日期。
- A scheduled update can not be cancelled (to proceed with migration even if exodus mode is activated while waiting for the migration; otherwise we would need to recover funds scheduled for migration with a separate procedure).
- 安排的更新无法取消（即使在等待迁移时激活了 Exodus 模式也要进行迁移；否则，我们需要使用单独的过程恢复计划迁移的资金）。
- Users can opt-in via a separate ZKSync operation: move specific token balance into a subtree on a special migration account. This subtree must also maintain and update counters for total balances per token.
- 用户可以通过单独的 ZKSync 操作选择加入：将特定令牌余额移动到一个特殊的迁移帐户的子树中。该子树还必须维护和更新每种令牌的总余额计数器。
- The migration account MUST have a dedicated hardcoded account_id (to specify).
- 迁移帐户必须具有专用的硬编码的账户 ID（待指定）。
- When the scheduled ETH block is reached, anybody MUST be able to seal the migration.
当达到计划的 ETH 块时，任何人都必须能够封闭迁移。
- After the migration is sealed, anybody MUST be able to transfer total balances for each token by providing a SNARK proof of the amounts from the migration account subtree.
- 迁移封闭后，任何人都必须能够通过提供来自迁移帐户子树金额的 SNARK 证明来转移每种令牌的总余额。
- When the migration is sealed, the contract enters exodus mode: whoever has not opted in can now exit. Thus, the root state will remain frozen.
- 当迁移被封存后，合约将进入“exodus 模式”：没有选择迁移的用户现在可以退出。因此，根状态将保持冻结。
- The new contract will read the latest state root directly from the old one (this is safe, because the root state is  frozen and can not be changed).
- 新合约将直接从旧合约中读取最新的状态根（这是安全的，因为根状态是冻结的，无法更改）。

## Todo / Questions / Unsorted 待办 / 问题 / 未分类

- introduce key hash?
- 引入密钥哈希？
- deploy keys separately?
- 分别部署密钥？
- unit conversion: different by token?
- 单位转换：不同代币不同？
- describe authorization for ERC20 transferFrom
- 描述 ERC20 transferFrom 的授权方式
- enforce max deposits/exits per block in the circuit
- 强制在电路中每个块的最大存款/退出数
