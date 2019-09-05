pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./VerificationKey.sol";
import "./Bytes.sol";

// GLOBAL TODOS:
// - check overflows

contract Franklin {
    VerificationKey verificationKey;
    Verifier verifier;
    Governance governance;

    // Expiration delta for priority request to be satisfied (in ETH blocks)
    uint256 constant EXPIRATION_DELTA = 250; // About 1 hour
    // chunks per block; each chunk has 8 bytes of public data
    uint256 constant BLOCK_SIZE = 10;
    // must fit into uint128
    uint256 constant MAX_VALUE = 2 ** 112 - 1;
    // ETH blocks
    uint256 constant EXPECT_VERIFICATION_IN = 8 * 60 * 100;
    // To make sure that all reverted blocks can be copied under block gas limit!
    uint256 constant MAX_UNVERIFIED_BLOCKS = 4 * 60 * 100;
    // Offchain address length.
    uint8 constant PUBKEY_HASH_LEN = 20;

    // Operations lengths

    uint256 constant NOOP_LENGTH = 1 * 8;
    uint256 constant DEPOSIT_LENGTH = 6 * 8;
    uint256 constant TRANSFER_TO_NEW_LENGTH = 5 * 8;
    uint256 constant PARTIAL_EXIT_LENGTH = 6 * 8;
    uint256 constant CLOSE_ACCOUNT_LENGTH = 1 * 8;
    uint256 constant TRANSFER_LENGTH = 2 * 8;
    uint256 constant FULL_EXIT_LENGTH = 7 * 8;

    // MARK: - Events

    // Event emitted when block is commited
    // Structure:
    // - blockNumber - number of commited block
    event BlockCommited(uint32 indexed blockNumber);
    // Event emitted when block is verified
    // Structure:
    // - blockNumber - number of verified block
    event BlockVerified(uint32 indexed blockNumber);

    // Event emitted when blocks are reverted
    // Structure:
    // - totalBlocksVerified - number of verified blocks
    // - totalBlocksCommited - number of commited blocks
    event BlocksReverted(
        uint32 indexed totalBlocksVerified,
        uint32 indexed totalBlocksCommited
    );

    // Event emitted when deposit operation comes into this contract
    // Structure:
    // - owner - sender
    // - tokenId - deposited token
    // - amount - deposited value
    // - franlkinAddress - address of Franklin account whtere deposit will be sent
    event OnchainDeposit(
        address indexed owner,
        uint16 tokenId,
        uint128 amount,
        bytes franklinAddress
    );

    // Event emitted when user send withdraw transaction from this contract
    // Structure:
    // - owner - sender
    // - tokenId - withdrawed token
    // - amount - withdrawed value
    event OnchainWithdrawal(
        address indexed owner,
        uint16 tokenId,
        uint128 amount
    );

    // New priority request event
    // Emitted when a request is placed into mapping
    // Params:
    // - opType - request type
    // - pubData - request data
    // - expirationBlock - the number of Ethereum block when request becomes expired
    event NewPriorityRequest(
        OpType indexed opType,
        bytes pubData,
        uint256 indexed expirationBlock
    );

    // MARK: - STORAGE

    // Possible token
    struct ValidatedTokenId {
        uint16 id;
    }

    // List of root-chain balances (per owner and tokenId) to withdraw
    mapping(address => mapping(uint16 => uint128)) public balancesToWithdraw;

    // Blocks

    // Total number of verified blocks
    // i.e. blocks[totalBlocksVerified] points at the latest verified block (block 0 is genesis)
    uint32 public totalBlocksVerified;

    // Total number of commited blocks
    // i.e. blocks[totalBlocksCommited] points at the latest commited block
    uint32 public totalBlocksCommited;

    // Block data (once per block)
    struct Block {
        // Hash of commitment the block circuit
        bytes32 commitment;
        // New root hash
        bytes32 stateRoot;
        // ETH block number at which this block was commited
        uint32 commitedAtBlock;
        // ETH block number at which this block was verified
        uint32 verifiedAtBlock;
        // Validator (aka block producer)
        address validator;
        // Index of the first operation to process for this block
        uint64 operationStartId;
        // Total number of operations to process for this block
        uint64 totalOperations;
        // Total number of priority operations for this block
        uint32 priorityOperations;
    }

    // List of blocks by Franklin blockId
    mapping(uint32 => Block) public blocks;

    // Types of franklin operations in blocks
    enum OpType {
        Noop,
        Deposit,
        TransferToNew,
        PartialExit,
        CloseAccount,
        Transfer,
        FullExit
    }

    // Onchain operations -- processed inside blocks (see docs)

    // Type of block processing operation holder
    enum OnchainOpType {
        Deposit,
        Withdrawal
    }

    // OnchainOp keeps a balance for processing the commited data in blocks, see docs
    struct OnchainOp {
        OnchainOpType opType;
        uint16 tokenId;
        address owner;
        uint128 amount;
    }

    // Total number of registered OnchainOps
    uint64 totalOnchainOps;

    // List of OnchainOps by index
    mapping(uint64 => OnchainOp) public onchainOps;

    // Priority Queue

    // Priority request op type and expiration block
    struct PriorityRequestParams {
        OpType opType;
        bytes pubData;
        uint256 expirationBlock;
    }

    // Requests params mapping (request id - (type, expiration block))
    // Contains op type and expiration block of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint32 => PriorityRequestParams) public priorityRequestsParams;
    // First priority request id
    uint32 public firstPriorityRequestId;
    // Total number of requests
    uint32 public totalPriorityRequests;

    // Reverting expired blocks

    // Total number of registered blocks to revert (see docs)
    uint32 totalBlocksToRevert;

    // List of blocks by revertBlockId (see docs)
    mapping(uint32 => Block) public blocksToRevert;

    // Flag indicating that exodus (mass exit) mode is triggered
    // Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

    // Flag indicating that a user has exited certain token balance (per owner and tokenId)
    mapping(address => mapping(uint16 => bool)) public exited;

    // // Migration

    // // Address of the new version of the contract to migrate accounts to
    // // Can be proposed by network governor
    // address public migrateTo;

    // // Migration deadline: after this ETH block number migration may happen with the contract
    // // entering exodus mode for all users who have not opted in for migration
    // uint32  public migrateByBlock;

    // // Flag for the new contract to indicate that the migration has been sealed
    // bool    public migrationSealed;

    // mapping (uint32 => bool) tokenMigrated;

    // MARK: - Constructor

    // Inits verifier and verification key contracts instances, sets genesis root, network governor
    constructor(
        address _verifierAddress,
        address _vkAddress,
        bytes32 _genesisRoot,
        address _governanceAddress
    ) public {
        verifier = Verifier(_verifierAddress);
        verificationKey = VerificationKey(_vkAddress);
        governance = Governance(_governanceAddress);

        blocks[0].stateRoot = _genesisRoot;
    }

    // MARK: - Priority Queue

    // Add request
    // Params:
    // - _opType - priority request type
    // - _pubData - request data
    function addPriorityRequest(OpType _opType, bytes memory _pubData) internal {
        uint256 expirationBlock = block.number + EXPIRATION_DELTA;
        priorityRequestsParams[firstPriorityRequestId+totalPriorityRequests] = PriorityRequestParams({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock
        });
        totalPriorityRequests++;

        emit NewPriorityRequest(
            _opType,
            _pubData,
            expirationBlock
        );
    }

    // Removes requests
    // Params:
    // - _count - number of requests to remove
    function removePriorityRequests(uint32 _count) internal {
        require(_count <= totalPriorityRequests, "Count of removed requests is higher than their count");

        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _count; i++) {
            delete priorityRequestsParams[i];
        }
        totalPriorityRequests -= _count;
        firstPriorityRequestId += _count;
    }

    // Removes certain requests
    // Params:
    // - _opType - operation type
    // - _anyRequestsCount - count of requests where to look certain requests for
    function removePriorityRequestsWithType(OpType _opType, uint32 _anyRequestsCount) internal {
        require(_anyRequestsCount <= totalPriorityRequests, "Count of removed requests is higher than their count");

        uint32 removingPriorityCount = 0;
        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _anyRequestsCount; i++) {
            if (priorityRequestsParams[i].opType == _opType) {
                delete priorityRequestsParams[i];
                removingPriorityCount++;
            }
        }
        totalPriorityRequests -= removingPriorityCount;
        firstPriorityRequestId += removingPriorityCount;
    }

    // Accrues balances from deposits from certain count of priority requests
    // Params:
    // - _anyRequestsCount - count of requests where to look deposit requests for
    function accrueBalancesFromDeposits(uint32 _anyRequestsCount) internal {
        require(_anyRequestsCount <= totalPriorityRequests, "Count of removed requests is higher than their count");

        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _anyRequestsCount; i++) {
            if (priorityRequestsParams[i].opType == OpType.Deposit) {
                bytes memory pubData = priorityRequestsParams[i].pubData;
                bytes memory owner = new bytes(20);
                for (uint256 j = 0; j < 20; ++j) {
                    owner[j] = pubData[j];
                }
                bytes memory token = new bytes(2);
                for (uint256 j = 0; j < 2; ++j) {
                    token[j] = pubData[20 + j];
                }
                bytes memory amount = new bytes(16);
                for (uint256 j = 0; j < 16; ++j) {
                    amount[j] = pubData[22 + j];
                }
                balancesToWithdraw[Bytes.bytesToAddress(owner)][Bytes.bytesToUInt16(token)] += Bytes.bytesToUInt128(amount);
            }
        }
    }

    // function scheduleMigration(address _migrateTo, uint32 _migrateByBlock) external {
    //     requireGovernor();
    //     require(migrateByBlock == 0, "migration in progress");
    //     migrateTo = _migrateTo;
    //     migrateByBlock = _migrateByBlock;
    // }

    // // Anybody MUST be able to call this function
    // function sealMigration() external {
    //     require(migrateByBlock > 0, "no migration scheduled");
    //     migrationSealed = true;
    //     exodusMode = true;
    // }

    // // Anybody MUST be able to call this function
    // function migrateToken(uint32 _tokenId, uint128 /*_amount*/, bytes calldata /*_proof*/) external {
    //     require(migrationSealed, "migration not sealed");
    //     requireValidToken(_tokenId);
    //     require(tokenMigrated[_tokenId]==false, "token already migrated");
    //     // TODO: check the proof for the amount
    //     // TODO: transfer ERC20 or ETH to the `migrateTo` address
    //     tokenMigrated[_tokenId] = true;

    //     require(false, "unimplemented");
    // }

    // MARK: - Root-chain operations

    // Deposit ETH (simply by sending it to the contract)
    // Params:
    // - _franklinAddr - receiver
    function depositETH(bytes calldata _franklinAddr) external payable {
        require(msg.value <= MAX_VALUE, "sorry Joe");
        registerDeposit(ValidatedTokenId(0), uint128(msg.value), _franklinAddr);
    }

    // Withdraw ETH
    // Params:
    // - _amount - amount to withdraw
    function withdrawETH(uint128 _amount) external {
        registerWithdrawal(ValidatedTokenId(0), _amount);
        msg.sender.transfer(_amount);
    }

    // Deposit ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount of token
    // - _franklinAddr - receiver
    function depositERC20(
        address _token,
        uint128 _amount,
        bytes calldata _franklinAddr
    ) external {
        require(
            IERC20(_token).transferFrom(msg.sender, address(this), _amount),
            "transfer failed"
        );
        ValidatedTokenId memory tokenId = ValidatedTokenId({
            id: governance.validateERC20Token(_token)
        });
        registerDeposit(tokenId, _amount, _franklinAddr);
    }

    // Withdraw ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external {
        ValidatedTokenId memory tokenId = ValidatedTokenId({
            id: governance.validateERC20Token(_token)
        });
        registerWithdrawal(tokenId, _amount);
        require(
            IERC20(_token).transfer(msg.sender, _amount),
            "transfer failed"
        );
    }

    // Register full exit request
    // Params:
    // - _franklinAddr - sender
    // - _eth_addr - receiver
    // - _token - token address
    // - _signature - user signature
    function registerFullExit(
        bytes calldata _franklinAddr,
        address _eth_addr,
        address _token,
        bytes calldata signature
    ) external {
        requireActive();
        ValidatedTokenId memory tokenId = ValidatedTokenId({
            id: governance.validateERC20Token(_token)
        });
        // Priority Queue request
        bytes memory pubData = _franklinAddr; // franklin address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(_eth_addr)); // eth address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId.id)); // token id
        pubData = Bytes.concat(pubData, signature); // signature
        addPriorityRequest(OpType.FullExit, pubData);
    }

    // Register deposit request
    // Params:
    // - _token - token by id
    // - _amount - token amount
    // - _franklinAddr - receiver
    function registerDeposit(
        ValidatedTokenId memory _token,
        uint128 _amount,
        bytes memory _franklinAddr
    ) internal {
        requireActive();

        emit OnchainDeposit(
            msg.sender,
            _token.id,
            _amount,
            _franklinAddr
        );

        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromAddress(msg.sender); // sender
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(_token.id)); // token id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt128(_amount)); // amount
        pubData = Bytes.concat(pubData, _franklinAddr); // franklin address
        addPriorityRequest(OpType.Deposit, pubData);
    }

    // Register withdrawal
    // Params:
    // - _token - token by id
    // - _amount - token amount
    function registerWithdrawal(ValidatedTokenId memory _token, uint128 _amount) internal {
        requireActive();
        require(
            balancesToWithdraw[msg.sender][_token.id] >= _amount,
            "insufficient balance"
        );
        balancesToWithdraw[msg.sender][_token.id] -= _amount;
        emit OnchainWithdrawal(
            msg.sender,
            _token.id,
            _amount
        );
    }

    // MARK: - Block commitment

    // Commit block
    // Params:
    // - _blockNumber - block number
    // - _feeAccount - account to collect fees
    // - _newRoot - new tree root
    // - _publicData - operations
    function commitBlock(
        uint32 _blockNumber,
        uint24 _feeAccount,
        bytes32 _newRoot,
        bytes calldata _publicData
    ) external {
        requireActive();
        governance.requireValidator(msg.sender);
        require(
            _blockNumber == totalBlocksCommited + 1,
            "only commit next block"
        );
        require(blockCommitmentExpired() == false, "commitment expired");
        require(
            totalBlocksCommited - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS,
            "too many commited"
        );

        // Enter exodus mode if needed
        require(!triggerExodusIfNeeded(), "Entered exodus mode");

        (uint64 startId, uint64 totalProcessed, uint32 priorityOperations) = commitOnchainOps(_publicData);

        bytes32 commitment = createBlockCommitment(
            _blockNumber,
            _feeAccount,
            blocks[_blockNumber - 1].stateRoot,
            _newRoot,
            _publicData
        );

        blocks[_blockNumber] = Block(
            commitment,
            _newRoot,
            uint32(block.number), // commited at
            0, // verified at
            msg.sender, // validator
            // onchain-ops
            startId,
            totalProcessed,
            priorityOperations
        );

        totalOnchainOps = startId + totalProcessed;

        totalBlocksCommited += 1;
        emit BlockCommited(_blockNumber);
    }
    
    // Returns block commitment
    // Params:
    // - _blockNumber - block number
    // - _feeAccount - account to collect fees
    // - _oldRoot - old tree root
    // - _newRoot - new tree root
    // - _publicData - operations
    function createBlockCommitment(
        uint32 _blockNumber,
        uint24 _feeAccount,
        bytes32 _oldRoot,
        bytes32 _newRoot,
        bytes memory _publicData
    ) internal pure returns (bytes32) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_blockNumber), uint256(_feeAccount))
        );
        hash = sha256(abi.encodePacked(hash, uint256(_oldRoot)));
        hash = sha256(abi.encodePacked(hash, uint256(_newRoot)));
        // public data is commited with padding (TODO: check assembly and optimize to avoid copying data)
        hash = sha256(
            abi.encodePacked(
                hash,
                _publicData
            )
        );

        return hash;
    }

    // Returns operations start id, onchain operations count, priority operations count
    // Params:
    // - _publicData - operations
    function commitOnchainOps(bytes memory _publicData)
        internal
        returns (uint64 onchainOpsStartId, uint64 processedOnchainOps, uint32 priorityOperations)
    {
        require(_publicData.length % 8 == 0, "pubdata.len % 8 != 0");

        onchainOpsStartId = totalOnchainOps;
        uint64 currentOnchainOp = totalOnchainOps;

        // NOTE: the stuff below is the most expensive and most frequently used part of the entire contract.
        // It is highly unoptimized and can be improved by an order of magnitude by getting rid of the subroutine,
        // using assembly, replacing ifs with mem lookups and other tricks
        // TODO: optimize
        uint256 currentPointer = 0;

        while (currentPointer < _publicData.length) {
            uint8 opType = uint8(_publicData[currentPointer]);
            (uint256 len, uint64 ops, uint32 priorityOps) = processOp(
                opType,
                currentPointer,
                _publicData,
                currentOnchainOp
            );
            currentPointer += len;
            processedOnchainOps += ops;
            priorityOperations += priorityOps;
        }
        require(
            currentPointer == _publicData.length,
            "last chunk exceeds pubdata"
        );
        return (onchainOpsStartId, processedOnchainOps, priorityOperations);
    }

    // Returns operation processed length, and indicators if it is onchain operation and if it is priority operation (1 if true)
    // Params:
    // - _opType - operation type
    // - _currentPointer - current pointer
    // - _publicData - operation data
    // - _newRoot - new tree root
    // - _currentOnchainOp - operation identifier
    function processOp(
        uint8 _opType,
        uint256 _currentPointer,
        bytes memory _publicData,
        uint64 _currentOnchainOp
    ) internal returns (uint256 processedLen, uint64 processedOnchainOps, uint32 priorityOperations) {
        uint256 opDataPointer = _currentPointer + 1;

        if (_opType == uint8(OpType.Noop)) return (NOOP_LENGTH, 0, 0); // noop
        if (_opType == uint8(OpType.TransferToNew)) return (TRANSFER_TO_NEW_LENGTH, 0, 0); // transfer_to_new
        if (_opType == uint8(OpType.Transfer)) return (TRANSFER_LENGTH, 0, 0); // transfer
        if (_opType == uint8(OpType.CloseAccount)) return (CLOSE_ACCOUNT_LENGTH, 0, 0); // close_account

        // TODO:
        // This deposit should not be placed to onchain ops. currently priority queue deposit
        // operations are responsible to return funds if there is a block revert, so onchain
        // deposit operation is unused
        if (_opType == uint8(OpType.Deposit)) {
            // to_account: 3, token: 2, amount: 16, fee: 2, new_pubkey_hash: 20

            uint16 tokenId = uint16(
                (uint256(uint8(_publicData[opDataPointer + 3])) << 8) +
                    uint256(uint8(_publicData[opDataPointer + 4]))
            );

            bytes memory amountBytes = new bytes(16);
            for (uint256 i = 0; i < 16; ++i) {
                amountBytes[i] = _publicData[opDataPointer + 5 + i];
            }
            uint128 amount = Bytes.bytesToUInt128(amountBytes);

            bytes memory franklinAddress = new bytes(PUBKEY_HASH_LEN);
            for (uint8 i = 0; i < PUBKEY_HASH_LEN; i++) {
                franklinAddress[i] = _publicData[opDataPointer + 9 + i];
            }

            governance.requireValidTokenId(tokenId);

            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Deposit,
                tokenId,
                Bytes.bytesToAddress(franklinAddress), // TODO: - this may fail if its length is not 20
                amount
            );
            return (DEPOSIT_LENGTH, 1, 1);
        }

        // partial_exit
        if (_opType == uint8(OpType.PartialExit)) {
            // pubdata account: 3, token: 2, amount: 16, fee: 2, eth_key: 20

            uint16 tokenId = uint16(
                (uint256(uint8(_publicData[opDataPointer + 3])) << 8) +
                    uint256(uint8(_publicData[opDataPointer + 4]))
            );

            bytes memory amountBytes = new bytes(16);
            for (uint256 i = 0; i < 16; ++i) {
                amountBytes[i] = _publicData[opDataPointer + 5 + i];
            }
            uint128 amount = Bytes.bytesToUInt128(amountBytes);

            bytes memory ethAddress = new bytes(20);
            for (uint256 i = 0; i < 20; ++i) {
                ethAddress[i] = _publicData[opDataPointer + 23 + i];
            }

            governance.requireValidTokenId(tokenId);
            
            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Withdrawal,
                tokenId,
                Bytes.bytesToAddress(ethAddress),
                amount
            );
            return (PARTIAL_EXIT_LENGTH, 1, 0);
        }

        // full_exit
        if (_opType == uint8(OpType.FullExit)) {
            // pubdata account: 3, eth_address: 20, token: 2, signature_hash: 20, full_amount: 16

            uint16 tokenId = uint16(
                (uint256(uint8(_publicData[opDataPointer + 23])) << 8) +
                    uint256(uint8(_publicData[opDataPointer + 24]))
            );

            bytes memory ethAddress = new bytes(20);
            for (uint256 i = 0; i < 20; ++i) {
                ethAddress[i] = _publicData[opDataPointer + 3 + i];
            }

            bytes memory signatureHash = new bytes(20);
            for (uint256 i = 0; i < 20; ++i) {
                signatureHash[i] = _publicData[opDataPointer + 25 + i];
            }

            bytes memory fullAmountBytes = new bytes(16);
            for (uint256 i = 0; i < 14; ++i) {
                fullAmountBytes[i] = _publicData[opDataPointer + 45 + i];
            }
            uint128 fullAmount = Bytes.bytesToUInt128(fullAmountBytes);

            governance.requireValidTokenId(tokenId);

            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Withdrawal,
                tokenId,
                Bytes.bytesToAddress(ethAddress),
                fullAmount
            );
            return (FULL_EXIT_LENGTH, 1, 1);
        }

        require(false, "unsupported op");
    }

    // MARK: - Block verification

    // Block verification.
    // Verify proof -> consummate onchain ops (accrue balances from withdrawls) -> remove priority requests
    // Params:
    // - blockNumber - block number
    // - proof - proof
    function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof)
        external
    {
        requireActive();
        governance.requireValidator(msg.sender);
        require(
            _blockNumber == totalBlocksVerified + 1,
            "only verify next block"
        );

        require(
            verifyBlockProof(proof, blocks[_blockNumber].commitment),
            "verification failed"
        );

        totalBlocksVerified += 1;
        
        consummateOnchainOps(_blockNumber);

        removePriorityRequests(blocks[_blockNumber].priorityOperations);

        emit BlockVerified(_blockNumber);
    }

    // Proof verification
    // Params:
    // - proof - block number
    // - commitment - block commitment
    function verifyBlockProof(uint256[8] memory proof, bytes32 commitment)
        internal
        view
        returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = verificationKey.getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(commitment) & mask;
        return verifier.Verify(vk, gammaABC, proof, inputs);
    }

    // If block verified the withdrawal onchain operations from it must be completed (user must have possibility to withdraw funds)
    // Params:
    // - _blockNumber - number of block
    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 start = blocks[_blockNumber].operationStartId;
        uint64 end = start + blocks[_blockNumber].totalOperations;
        for (uint64 current = start; current < end; ++current) {
            OnchainOp memory op = onchainOps[current];
            if (op.opType == OnchainOpType.Withdrawal) {
                // withdrawal was successful, accrue balance
                balancesToWithdraw[op.owner][op.tokenId] += op.amount;
            }
            delete onchainOps[current];
        }
    }

    // MARK: - Reverting commited blocks

    // Fill blocksToRevert mapping and set totalBlocksToRevert value, depending on the difference between totalBlocksVerified and totalBlocksCommited
    function revertExpiredBlocks() public {
        require(blockCommitmentExpired(), "not expired");
        emit BlocksReverted(totalBlocksVerified, totalBlocksCommited);
        uint32 total = totalBlocksCommited - totalBlocksVerified;
        for (uint32 i = 0; i < total; i++) {
            blocksToRevert[totalBlocksToRevert +
                i] = blocks[totalBlocksVerified + i + 1];
            delete blocks[totalBlocksVerified + i + 1];
        }
        totalBlocksToRevert += total;
    }

    // Delete block onchain operations, accrue balances from deposits and remove deposit priority requests from its mapping
    // Params:
    // - _revertedBlockId - block id
    function revertBlock(uint32 _revertedBlockId) public {
        Block memory reverted = blocksToRevert[_revertedBlockId];
        require(reverted.commitedAtBlock > 0, "block not found");
        require(reverted.priorityOperations <= totalPriorityRequests, "Count of removed requests is higher than their count");

        uint64 current = reverted.operationStartId;
        uint64 end = current + reverted.totalOperations;
        while (current < end) {
            delete onchainOps[current];
        }

        // accrue balances from deposits and delete deposits in priority requests
        accrueBalancesFromDeposits(reverted.priorityOperations);
        removePriorityRequestsWithType(OpType.Deposit, reverted.priorityOperations);

        delete blocksToRevert[_revertedBlockId];
    }

    // MARK: - Exodus mode

    // Returns bool flag. True if the Exodus mode must be entered
    function triggerExodusIfNeeded() internal returns (bool) {
        if (block.number >= priorityRequestsParams[firstPriorityRequestId].expirationBlock) {
            exodusMode = true;
            revertExpiredBlocks();
            for (uint32 i = 0; i < totalBlocksToRevert; i++) {
                revertBlock(i);
            }
            return true;
        } else {
            return false;
        }
    }

    // Withdraw token from Franklin to root chain
    // Params:
    // - _tokenId - verified token id
    // - _owners - all owners
    // - _amounts - amounts for owners
    // - _proof - proof
    function exit(
        uint16 _tokenId,
        address[] calldata _owners,
        uint128[] calldata _amounts,
        uint256[8] calldata /*_proof*/
    ) external {
        require(exodusMode, "must be in exodus mode");
        require(_owners.length == _amounts.length, "|owners| != |amounts|");

        for (uint256 i = 0; i < _owners.length; i++) {
            require(exited[_owners[i]][_tokenId] == false, "already exited");
        }

        // TODO: verify the proof that all users have the specified amounts of this token in the latest state

        for (uint256 i = 0; i < _owners.length; i++) {
            balancesToWithdraw[_owners[i]][_tokenId] += _amounts[i];
            exited[_owners[i]][_tokenId] = true;
        }
    }

    // Internal helpers

    function requireActive() internal view {
        require(!exodusMode, "exodus mode");
    }

    function blockCommitmentExpired() internal view returns (bool) {
        return
            totalBlocksCommited > totalBlocksVerified &&
                block.number >
                blocks[totalBlocksVerified + 1].commitedAtBlock +
                    EXPECT_VERIFICATION_IN;
    }

}
