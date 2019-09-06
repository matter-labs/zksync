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
    uint256 constant PRIORITY_EXPIRATION = 250; // About 1 hour
    // chunks per block; each chunk has 8 bytes of public data
    uint256 constant BLOCK_SIZE = 10;
    // must fit into uint128
    uint256 constant MAX_VALUE = 2 ** 112 - 1;
    // ETH blocks
    uint256 constant EXPECT_VERIFICATION_IN = 8 * 60 * 100;
    // To make sure that all reverted blocks can be copied under block gas limit!
    uint256 constant MAX_UNVERIFIED_BLOCKS = 4 * 60 * 100;
    // Franklin chain address length.
    uint8 constant PUBKEY_HASH_LEN = 20;
    // Signature length.
    uint8 constant SIGNATURE_LEN = 32;

    // Operations lengths

    uint256 constant NOOP_LENGTH = 1 * 8;
    uint256 constant DEPOSIT_LENGTH = 6 * 8;
    uint256 constant TRANSFER_TO_NEW_LENGTH = 5 * 8;
    uint256 constant PARTIAL_EXIT_LENGTH = 6 * 8;
    uint256 constant CLOSE_ACCOUNT_LENGTH = 1 * 8;
    uint256 constant TRANSFER_LENGTH = 2 * 8;
    uint256 constant FULL_EXIT_LENGTH = 10 * 8;

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

    // Exodus mode entered event
    event ExodusMode();

    // MARK: - STORAGE

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
        // Validator (aka block producer)
        address validator;
        // Index of the first operation to process for this block
        uint64 operationStartId;
        // Total number of operations to process for this block
        uint64 onchainOperations;
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
    // enum OnchainOpType {
    //     Deposit,
    //     Withdrawal
    // }

    // Operation keeps a balance for processing the commited data in blocks, see docs
    // struct Operation {
    //     OpType opType;
    //     uint16 tokenId;
    //     address owner;
    //     uint128 amount;
    // }
    struct Operation {
        OpType opType;
        bytes pubData;
        uint256 expirationBlock;
    }

    // Total number of registered OnchainOps
    uint64 totalOnchainOps;

    // List of OnchainOps by index
    mapping(uint64 => Operation) public onchainOps;

    // Priority Queue

    // Priority Requests  mapping (request id - operation)
    // Contains op type, pubdata and expiration block of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint32 => Operation) public priorityRequests;
    // First priority request id
    uint32 public firstPriorityRequestId;
    // Total number of requests
    uint32 public totalPriorityRequests;

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
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;
        priorityRequests[firstPriorityRequestId+totalPriorityRequests] = Operation({
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
        require(_count <= totalPriorityRequests, "c1"); // c1 - count is heigher than total priority requests count

        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _count; i++) {
            delete priorityRequests[i];
        }
        totalPriorityRequests -= _count;
        firstPriorityRequestId += _count;
    }

    // Accrues balances from deposits from block and removes requests.
    // WARNING: Only for Exodus mode
    // Params:
    // - _anyRequestsCount - count of requests where to look deposit requests for
    function accrueBalancesForDepositsFromBlockPriorityOpsAndRemoveItsRequests(uint32 _count) internal {
        require(_count <= totalPriorityRequests, "c2"); // c2 - count is heigher than total priority requests count

        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _count; i++) {
            if (priorityRequests[i].opType == OpType.Deposit) {
                bytes memory pubData = priorityRequests[i].pubData;
                bytes memory owner = new bytes(20);
                for (uint256 j = 0; j < 20; ++j) {
                    owner[j] = pubData[j];
                }
                uint16 token = uint16(
                    (uint256(uint8(pubData[20])) << 8) +
                        uint256(uint8(pubData[21]))
                );
                bytes memory amount = new bytes(16);
                for (uint256 j = 0; j < 16; ++j) {
                    amount[j] = pubData[22 + j];
                }
                balancesToWithdraw[Bytes.bytesToAddress(owner)][token] += Bytes.bytesToUInt128(amount);
            }
            delete priorityRequests[i];
        }
        totalPriorityRequests -= _count;
        firstPriorityRequestId += _count;
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
        require(msg.value <= MAX_VALUE, "d1"); // d1 - deposit value is heighr
        registerDeposit(0, uint128(msg.value), _franklinAddr);
    }

    // Withdraw ETH
    // Params:
    // - _amount - amount to withdraw
    function withdrawETH(uint128 _amount) external {
        registerWithdrawal(0, _amount);
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
            "token transfer failed deposit"
        );
        uint16 tokenId = governance.validateERC20Token(_token);
        registerDeposit(tokenId, _amount, _franklinAddr);
    }

    // Withdraw ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external {
        uint16 tokenId = governance.validateERC20Token(_token);
        registerWithdrawal(tokenId, _amount);
        require(
            IERC20(_token).transfer(msg.sender, _amount),
            "token transfer failed withdraw"
        );
    }

    // Register full exit request
    // Params:
    // - _franklinAddr - sender
    // - _token - token address
    // - _signature - user signature
    function registerFullExit(
        bytes calldata _franklinAddr,
        address _token,
        bytes calldata _signature
    ) external {
        requireActive();
        require(_franklinAddr.length == PUBKEY_HASH_LEN, "wrong pubkey length");
        require(_signature.length == SIGNATURE_LEN, "wrong signature length");

        uint16 tokenId = governance.validateERC20Token(_token);
        // Priority Queue request
        bytes memory pubData = _franklinAddr; // franklin address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(msg.sender)); // eth address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId)); // token id
        pubData = Bytes.concat(pubData, _signature); // signature
        addPriorityRequest(OpType.FullExit, pubData);
    }

    // Register deposit request
    // Params:
    // - _token - token by id
    // - _amount - token amount
    // - _franklinAddr - receiver
    function registerDeposit(
        uint16 _token,
        uint128 _amount,
        bytes memory _franklinAddr
    ) internal {
        requireActive();

        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromAddress(msg.sender); // sender
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(_token)); // token id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt128(_amount)); // amount
        pubData = Bytes.concat(pubData, _franklinAddr); // franklin address
        addPriorityRequest(OpType.Deposit, pubData);

        emit OnchainDeposit(
            msg.sender,
            _token.id,
            _amount,
            _franklinAddr
        );
    }

    // Register withdrawal
    // Params:
    // - _token - token by id
    // - _amount - token amount
    function registerWithdrawal(uint16 _token, uint128 _amount) internal {
        requireActive();
        require(
            balancesToWithdraw[msg.sender][_token] >= _amount,
            "insufficient balance withdraw"
        );

        balancesToWithdraw[msg.sender][_token] -= _amount;

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
        require(!triggerExodusIfNeeded(), "entered exodus mode");
        requireActive();
        require(
            governance.isValidator(msg.sender),
            "not a validator in commit");
        require(
            _blockNumber == totalBlocksCommited + 1,
            "only commit next block"
        );
        require(!triggerRevertIfBlockCommitmentExpired(), "commitment expired");
        require(
            totalBlocksCommited - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS,
            "too many commited"
        );

        (uint64 startId, uint64 totalProcessed, uint32 priorityCount) = collectOnchainOps(_publicData);
        if (blockPriorityOperationsValid(startId, totalProcessed, priorityCount)) {
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
                msg.sender, // validator
                // onchain-ops
                startId,
                totalProcessed,
                priorityCount
            );

            totalOnchainOps = startId + totalProcessed;

            totalBlocksCommited += 1;
            emit BlockCommited(_blockNumber);
        } else {
            removeOnchainOps(startId, totalProcessed);
            revert("wrong onchain ops");
        }
    }

    // Returns operations start id, onchain operations count, priority operations count
    // Params:
    // - _publicData - operations
    function collectOnchainOps(bytes memory _publicData)
        internal
        returns (uint64 onchainOpsStartId, uint64 processedOnchainOps, uint32 priorityCount)
    {
        require(_publicData.length % 8 == 0, "pubdata.len % 8 != 0");

        onchainOpsStartId = totalOnchainOps;
        uint64 currentOnchainOp = totalOnchainOps;

        uint256 currentPointer = 0;

        while (currentPointer < _publicData.length) {
            uint8 opType = uint8(_publicData[currentPointer]);
            (uint256 len, uint64 ops, uint32 priority) = processOp(
                opType,
                currentPointer,
                _publicData,
                currentOnchainOp
            );
            currentPointer += len;
            processedOnchainOps += ops;
            priorityCount += priority;
        }
        require(
            currentPointer == _publicData.length,
            "last chunk exceeds pubdata"
        );
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
    ) internal returns (uint256 processedLen, uint64 processedOnchainOps, uint32 priorityCount) {
        uint256 opDataPointer = _currentPointer + 1;

        if (_opType == uint8(OpType.Noop)) return (NOOP_LENGTH, 0, 0);
        if (_opType == uint8(OpType.TransferToNew)) return (TRANSFER_TO_NEW_LENGTH, 0, 0);
        if (_opType == uint8(OpType.Transfer)) return (TRANSFER_LENGTH, 0, 0);
        if (_opType == uint8(OpType.CloseAccount)) return (CLOSE_ACCOUNT_LENGTH, 0, 0);

        if (_opType == uint8(OpType.Deposit)) {
            onchainOps[_currentOnchainOp] = Operation(
                OpType.Deposit,
                Bytes.slice(_publicData, opDataPointer + 3, 38),
                0
            );
            return (DEPOSIT_LENGTH, 1, 1);
        }

        if (_opType == uint8(OpType.PartialExit)) {
            onchainOps[_currentOnchainOp] = Operation(
                OpType.PartialExit,
                Bytes.slice(_publicData, opDataPointer + 3, 40),
                0
            );
            return (PARTIAL_EXIT_LENGTH, 1, 0);
        }

        if (_opType == uint8(OpType.FullExit)) {
            onchainOps[_currentOnchainOp] = Operation(
                OpType.FullExit,
                Bytes.slice(_publicData, opDataPointer + 3, 70),
                0
            );
            return (FULL_EXIT_LENGTH, 1, 1);
        }

        revert("unsupported op");
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

    // Check if blocks' priority operations are valid
    // Params:
    // - startId - onchain op start id
    // - totalProcessed - how many ops are procceeded
    // - priorityCount - priority ops count
    function blockPriorityOperationsValid(uint64 startId, uint64 totalProcessed, uint32 priorityCount) internal view returns (bool) {
        require(priorityCount <= totalPriorityRequests, "too much priority requests");
        
        uint64 start = startId;
        uint64 end = start + totalProcessed;

        Operation[] memory priorityOps;
        
        uint32 counter = 0;
        for (uint64 current = start; current < end; ++current) {
            Operation memory op = onchainOps[current];
            if (op.opType == OpType.FullExit || op.opType == OpType.Deposit) {
                priorityOps[counter] = op;
                counter++;
            }
        }

        if (counter != priorityCount) {
            return false;
        }
        
        for (uint32 i = 0; i < priorityCount; i++) {
            if (!comparePriorityOps(priorityOps[i], i+firstPriorityRequestId)) {
                return false;
            }
        }
        return true;
    }

    // Compare operation from block with corresponding priority requests' operation
    // Params:
    // - onchainOp - operation from block
    // - priorityRequestId - priority request id
    function comparePriorityOps(Operation memory onchainOp, uint32 priorityRequestId) internal view returns (bool) {
        bytes memory priorityPubData;
        bytes memory onchainPubData;
        OpType operation;
        if (onchainOp.opType == OpType.Deposit && priorityRequests[priorityRequestId].opType == OpType.Deposit) {
            priorityPubData = Bytes.slice(priorityRequests[priorityRequestId].pubData, 20, PUBKEY_HASH_LEN + 18);
            onchainPubData = onchainOp.pubData;
            operation = OpType.Deposit;
        } else if (onchainOp.opType == OpType.FullExit && priorityRequests[priorityRequestId].opType == OpType.FullExit) {
            priorityPubData = Bytes.slice(priorityRequests[priorityRequestId].pubData, PUBKEY_HASH_LEN, 54);
            onchainPubData = Bytes.slice(onchainOp.pubData, 0, 54);
            operation = OpType.FullExit;
        } else {
            revert("Wrong operation");
        }
        return (priorityPubData.length > 0) &&
            (keccak256(onchainPubData) == keccak256(priorityPubData));
    }

    // Remove some onchain ops (for example in case of wrong priority comparison)
    // Params:
    // - startId - onchain op start id
    // - totalProcessed - how many ops are procceeded
    function removeOnchainOps(uint64 startId, uint64 totalProcessed) internal {
        uint64 start = startId;
        uint64 end = start + totalProcessed;

        for (uint64 current = start; current < end; ++current) {
            delete onchainOps[current];
        }
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
        require(
            governance.isValidator(msg.sender),
            "not a validator in verify");
        require(
            _blockNumber == totalBlocksVerified + 1,
            "only verify next block"
        );

        require(
            verifyBlockProof(proof, blocks[_blockNumber].commitment),
            "verification failed"
        );
        
        consummateOnchainOps(_blockNumber);

        removePriorityRequests(blocks[_blockNumber].priorityOperations);

        totalBlocksVerified += 1;

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

    // If block verified the onchain operations from it must be completed (user must have possibility to withdraw funds if withdrawed)
    // Params:
    // - _blockNumber - number of block
    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 start = blocks[_blockNumber].operationStartId;
        uint64 end = start + blocks[_blockNumber].onchainOperations;
        for (uint64 current = start; current < end; ++current) {
            Operation memory op = onchainOps[current];
            if (op.opType == OpType.PartialExit) {
                // partial exit was successful, accrue balance
                uint16 tokenId = uint16(
                    (uint256(uint8(op.pubData[0])) << 8) +
                        uint256(uint8(op.pubData[1]))
                );

                bytes memory amountBytes = new bytes(16);
                for (uint256 i = 0; i < 16; ++i) {
                    amountBytes[i] = op.pubData[2 + i];
                }
                uint128 amount = Bytes.bytesToUInt128(amountBytes);

                bytes memory ethAddress = new bytes(20);
                for (uint256 i = 0; i < 20; ++i) {
                    ethAddress[i] = op.pubData[20 + i];
                }
                balancesToWithdraw[Bytes.bytesToAddress(ethAddress)][tokenId] += amount;
            }
            if (op.opType == OpType.FullExit) {
                // full exit was successful, accrue balance
                uint16 tokenId = uint16(
                    (uint256(uint8(op.pubData[20])) << 8) +
                        uint256(uint8(op.pubData[21]))
                );

                bytes memory amountBytes = new bytes(16);
                for (uint256 i = 0; i < 16; ++i) {
                    amountBytes[i] = op.pubData[54 + i];
                }
                uint128 amount = Bytes.bytesToUInt128(amountBytes);

                bytes memory ethAddress = new bytes(20);
                for (uint256 i = 0; i < 20; ++i) {
                    ethAddress[i] = op.pubData[i];
                }
                balancesToWithdraw[Bytes.bytesToAddress(ethAddress)][tokenId] += amount;
            }
            delete onchainOps[current];
        }
    }

    // MARK: - Reverting commited blocks

    // Check that commitment is expired and revert blocks
    function triggerRevertIfBlockCommitmentExpired() public returns (bool) {
        if (totalBlocksCommited > totalBlocksVerified &&
                block.number >
                blocks[totalBlocksVerified + 1].commitedAtBlock +
                    EXPECT_VERIFICATION_IN) {
            revertBlocks(false);
            return true;
        }
        return false;
    }

    // Revert blocks
    function revertBlocks(bool fromExodus) internal {
        for (uint32 i = totalBlocksVerified; i < totalBlocksCommited-1; i++) {
            Block memory reverted = blocks[i];
            if (fromExodus) {
                // in case of exodus accrue balances from deposits
                accrueBalancesForDepositsFromBlockPriorityOpsAndRemoveItsRequests(reverted.priorityOperations);
            }
            revertBlock(reverted);
            delete blocks[i];
        }
        totalBlocksCommited -= totalBlocksCommited - totalBlocksVerified;
        emit BlocksReverted(totalBlocksVerified, totalBlocksCommited);
    }

    // Delete block onchain operations, accrue balances from deposits and remove deposit priority requests from its mapping
    // Params:
    // - _revertedBlockId - block id
    function revertBlock(Block memory reverted) internal {
        require(reverted.commitedAtBlock > 0, "block not found");
        require(reverted.priorityOperations <= totalPriorityRequests, "priority count too large in revert");
        removeOnchainOps(reverted.operationStartId, reverted.onchainOperations);
    }

    // MARK: - Exodus mode

    // Check that current state not is exodus mode
    function requireActive() internal view {
        require(!exodusMode, "exodus mode");
    }

    // Returns bool flag. True if the Exodus mode must be entered
    function triggerExodusIfNeeded() internal returns (bool) {
        if (block.number >= priorityRequests[firstPriorityRequestId].expirationBlock) {
            exodusMode = true;
            revertBlocks(true);
            emit ExodusMode();
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

}
