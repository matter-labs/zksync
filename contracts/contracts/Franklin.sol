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

    // Base gas for transaction
    uint256 constant FEE_COEFF = 21000;
    // Base gas cost for transaction
    uint256 constant BASE_GAS = 21000;
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
    // - fee - validator fee
    event NewPriorityRequest(
        OpType indexed opType,
        bytes pubData,
        uint256 indexed expirationBlock,
        uint256 fee
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
        // Index of the first operation to process for this block
        uint64 operationStartId;
        // Total number of operations to process for this block
        uint64 onchainOperations;
        // Total number of priority operations for this block
        uint32 priorityOperations;
        // ETH block number at which this block was commited
        uint32 commitedAtBlock;
        // Hash of commitment the block circuit
        bytes32 commitment;
        // New root hash
        bytes32 stateRoot;
        // Validator (aka block producer)
        address validator;
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

    // Operation keeps a balance for processing the commited data in blocks, see docs
    struct OnchainOperation {
        OpType opType;
        bytes pubData;
    }

    // Total number of registered OnchainOps
    uint64 totalOnchainOps;

    // List of OnchainOps by index
    mapping(uint64 => OnchainOperation) public onchainOps;

    // Priority Queue

    // Operation keeps a balance for processing the commited data in blocks, see docs
    struct PriorityOperation {
        OpType opType;
        bytes pubData;
        uint256 expirationBlock;
        uint256 fee;
    }

    // Priority Requests  mapping (request id - operation)
    // Contains op type, pubdata and expiration block of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint32 => PriorityOperation) public priorityRequests;
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
    // - _fee - validatorFee
    // - _pubData - request data
    function addPriorityRequest(
        OpType _opType,
        uint256 _fee,
        bytes memory _pubData
    ) internal {
        // Expiration block is: current block number + priority expiration delta
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;

        emit NewPriorityRequest(
            _opType,
            _pubData,
            expirationBlock,
            _fee
        );

        priorityRequests[firstPriorityRequestId+totalPriorityRequests] = PriorityOperation({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock,
            fee: _fee
        });
        totalPriorityRequests++;
    }

    // Pays validator fee and removes requests
    // Params:
    // - _count - number of requests to remove
    // - _validator - address to pay fee
    function payValidatorFeeAndRemovePriorityRequests(uint32 _count, address payable _validator) internal {
        require(
            _count <= totalPriorityRequests,
            "rprcnt"
        ); // rprcnt - count is heigher than total priority requests count

        uint256 totalFee = 0;
        for (uint32 i = firstPriorityRequestId; i < firstPriorityRequestId + _count; i++) {
            totalFee += priorityRequests[i].fee;
            delete priorityRequests[i];
        }
        totalPriorityRequests -= _count;
        firstPriorityRequestId += _count;

        _validator.transfer(totalFee);
    }

    // Accrues balances from deposits from block and removes requests.
    // WARNING: Only for Exodus mode
    // Params:
    // - _anyRequestsCount - count of requests where to look deposit requests for
    function accrueBalancesForDepositsFromBlockPriorityOpsAndRemoveItsRequests(uint32 _count) internal {
        require(
            _count <= totalPriorityRequests,
            "abfcnt"
        ); // abfcnt - count is heigher than total priority requests count

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
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee,
            "detlwv"
        ); // derlwv - Not enough ETH provided to pay the fee

        uint128 amount = uint128(msg.value-fee);
        require(
            amount <= MAX_VALUE,
            "detamt"
        ); // detamt - deposit amount value is heigher than Franklin is able to process

        registerDeposit(0, amount, fee, _franklinAddr);
    }

    // Withdraw ETH
    // Params:
    // - _amount - amount to withdraw
    function withdrawETH(uint128 _amount) external {
        requireActive();
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
    ) external payable {
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee,
            "derlwv"
        ); // derlwv - Not enough ETH provided to pay the fee

        require(
            IERC20(_token).transferFrom(msg.sender, address(this), _amount),
            "dertrf"
        ); // dertrf - token transfer failed deposit

        uint16 tokenId = governance.validateERC20Token(_token);
        registerDeposit(tokenId, _amount, fee, _franklinAddr);
    }

    // Withdraw ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external {
        requireActive();
        uint16 tokenId = governance.validateERC20Token(_token);
        registerWithdrawal(tokenId, _amount);
        require(
            IERC20(_token).transfer(msg.sender, _amount),
            "wertrf"
        ); // wertrf - token transfer failed withdraw
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
    ) external payable {
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee,
            "derlwv"
        ); // derlwv - Not enough ETH provided to pay the fee
        require(
            _franklinAddr.length == PUBKEY_HASH_LEN,
            "rfepkl"
        ); // rfepkl - wrong pubkey length
        require(
            _signature.length == SIGNATURE_LEN,
            "rfesnl"
        ); // rfesnl - wrong signature length

        uint16 tokenId = governance.validateERC20Token(_token);
        // Priority Queue request
        bytes memory pubData = _franklinAddr; // franklin address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(msg.sender)); // eth address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId)); // token id
        pubData = Bytes.concat(pubData, _signature); // signature
        addPriorityRequest(OpType.FullExit, fee, pubData);
    }

    // Register deposit request
    // Params:
    // - _token - token by id
    // - _amount - token amount
    // - _fee - validator fee
    // - _franklinAddr - receiver
    function registerDeposit(
        uint16 _token,
        uint128 _amount,
        uint256 _fee,
        bytes memory _franklinAddr
    ) internal {
        emit OnchainDeposit(
            msg.sender,
            _token,
            _amount,
            _franklinAddr
        );

        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromAddress(msg.sender); // sender
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(_token)); // token id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt128(_amount)); // amount
        pubData = Bytes.concat(pubData, _franklinAddr); // franklin address
        addPriorityRequest(OpType.Deposit, _fee, pubData);
    }

    // Register withdrawal
    // Params:
    // - _token - token by id
    // - _amount - token amount
    function registerWithdrawal(uint16 _token, uint128 _amount) internal {
        require(
            balancesToWithdraw[msg.sender][_token] >= _amount,
            "rwthamt"
        ); // rwthamt - insufficient balance withdraw

        balancesToWithdraw[msg.sender][_token] -= _amount;

        emit OnchainWithdrawal(
            msg.sender,
            _token,
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
        require(
            _blockNumber == totalBlocksCommited + 1,
            "cbkbnr"
        ); // rwthamt - only commit next block
        require(
            totalBlocksCommited - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS,
            "cbkmub"
        ); // cbkmub - too many commited
        require(
            governance.isValidator(msg.sender),
            "cbkvlr"
        ); // cbkvlr - not a validator in commit
        require(
            !triggerRevertIfBlockCommitmentExpired(),
            "cbkcex"
        ); // cbkcex - commitment expired
        require(
            !triggerExodusIfNeeded(),
            "cbkexm"
        ); // cbkexm - entered exodus mode

        (uint64 startId, uint64 totalProcessed, uint32 priorityCount) = collectOnchainOps(_publicData);
        if (areBlockPriorityOperationsValid(startId, totalProcessed, priorityCount)) {
            bytes32 commitment = createBlockCommitment(
                _blockNumber,
                _feeAccount,
                blocks[_blockNumber - 1].stateRoot,
                _newRoot,
                _publicData
            );

            blocks[_blockNumber] = Block(
                // onchain-ops
                startId,
                totalProcessed,
                priorityCount,
                uint32(block.number), // commited at
                commitment,
                _newRoot,
                msg.sender // validator
            );

            totalOnchainOps = startId + totalProcessed;

            totalBlocksCommited += 1;
            emit BlockCommited(_blockNumber);
        } else {
            removeOnchainOps(startId, totalProcessed);
            revert("cbkwoo"); // cbkwoo - wrong onchain ops
        }
    }

    // Returns operations start id, onchain operations count, priority operations count
    // Params:
    // - _publicData - operations
    function collectOnchainOps(bytes memory _publicData)
        internal
        returns (uint64 onchainOpsStartId, uint64 processedOnchainOps, uint32 priorityCount)
    {
        require(
            _publicData.length % 8 == 0,
            "coopln"
        ); // coopdn - pubdata.len % 8 != 0

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
            "cooprn"
        ); // cooprn - last chunk exceeds pubdata
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
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.Deposit,
                Bytes.slice(_publicData, opDataPointer + 3, 38)
            );
            return (DEPOSIT_LENGTH, 1, 1);
        }

        if (_opType == uint8(OpType.PartialExit)) {
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.PartialExit,
                Bytes.slice(_publicData, opDataPointer + 3, 40)
            );
            return (PARTIAL_EXIT_LENGTH, 1, 0);
        }

        if (_opType == uint8(OpType.FullExit)) {
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.FullExit,
                Bytes.slice(_publicData, opDataPointer + 3, 70)
            );
            return (FULL_EXIT_LENGTH, 1, 1);
        }

        revert("popuop"); // popuop - unsupported op
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
    // - _startId - onchain op start id
    // - _totalProcessed - how many ops are procceeded
    // - _priorityCount - priority ops count
    function areBlockPriorityOperationsValid(uint64 _startId, uint64 _totalProcessed, uint32 _priorityCount) internal view returns (bool) {
        require(
            _priorityCount <= totalPriorityRequests,
            "bpoprc"
        ); // bpoprc - too much priority requests
        
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;

        OnchainOperation[] memory priorityOps;
        
        uint32 counter = 0;
        for (uint64 current = start; current < end; ++current) {
            OnchainOperation memory op = onchainOps[current];
            if (op.opType == OpType.FullExit || op.opType == OpType.Deposit) {
                priorityOps[counter] = op;
                counter++;
            }
        }

        if (counter != _priorityCount) {
            return false;
        }
        
        for (uint32 i = 0; i < _priorityCount; i++) {
            if (!comparePriorityOps(priorityOps[i], i+firstPriorityRequestId)) {
                return false;
            }
        }
        return true;
    }

    // Compare operation from block with corresponding priority requests' operation
    // Params:
    // - _onchainOp - operation from block
    // - _priorityRequestId - priority request id
    function comparePriorityOps(OnchainOperation memory _onchainOp, uint32 _priorityRequestId) internal view returns (bool) {
        bytes memory priorityPubData;
        bytes memory onchainPubData;
        OpType operation;
        if (_onchainOp.opType == OpType.Deposit && priorityRequests[_priorityRequestId].opType == OpType.Deposit) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, 20, PUBKEY_HASH_LEN + 18);
            onchainPubData = _onchainOp.pubData;
            operation = OpType.Deposit;
        } else if (_onchainOp.opType == OpType.FullExit && priorityRequests[_priorityRequestId].opType == OpType.FullExit) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, PUBKEY_HASH_LEN, 54);
            onchainPubData = Bytes.slice(_onchainOp.pubData, 0, 54);
            operation = OpType.FullExit;
        } else {
            revert("cpowop"); // cpowop - wrong operation
        }
        return (priorityPubData.length > 0) &&
            (keccak256(onchainPubData) == keccak256(priorityPubData));
    }

    // Remove some onchain ops (for example in case of wrong priority comparison)
    // Params:
    // - _startId - onchain op start id
    // - _totalProcessed - how many ops are procceeded
    function removeOnchainOps(uint64 _startId, uint64 _totalProcessed) internal {
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;

        for (uint64 current = start; current < end; ++current) {
            delete onchainOps[current];
        }
    }

    // MARK: - Block verification

    // Block verification.
    // Verify proof -> consummate onchain ops (accrue balances from withdrawls) -> remove priority requests
    // Params:
    // - blockNumber - block number
    // - _proof - proof
    function verifyBlock(uint32 _blockNumber, uint256[8] calldata _proof)
        external
    {
        requireActive();
        require(
            _blockNumber == totalBlocksVerified + 1,
            "vbknbk"
        ); // vbknbk - only verify next block
        require(
            governance.isValidator(msg.sender),
            "vbkvdr"
        ); // vbkvdr - not a validator in verify
        require(
            verifyBlockProof(_proof, blocks[_blockNumber].commitment),
            "vbkvbp"
        ); // vbkvbp - verification failed
        
        consummateOnchainOps(_blockNumber);

        payValidatorFeeAndRemovePriorityRequests(
            blocks[_blockNumber].priorityOperations,
            address(uint160(blocks[_blockNumber].validator))
        );

        totalBlocksVerified += 1;

        emit BlockVerified(_blockNumber);
    }

    // Proof verification
    // Params:
    // - _proof - block number
    // - _commitment - block commitment
    function verifyBlockProof(uint256[8] memory _proof, bytes32 _commitment)
        internal
        view
        returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = verificationKey.getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(_commitment) & mask;
        return verifier.Verify(vk, gammaABC, _proof, inputs);
    }

    // If block verified the onchain operations from it must be completed (user must have possibility to withdraw funds if withdrawed)
    // Params:
    // - _blockNumber - number of block
    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 start = blocks[_blockNumber].operationStartId;
        uint64 end = start + blocks[_blockNumber].onchainOperations;
        for (uint64 current = start; current < end; ++current) {
            OnchainOperation memory op = onchainOps[current];
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
    // Params:
    // - _fromExodus - if revert caused by exodus mode
    function revertBlocks(bool _fromExodus) internal {
        for (uint32 i = totalBlocksVerified; i < totalBlocksCommited-1; i++) {
            Block memory reverted = blocks[i];
            if (_fromExodus) {
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
    // - _reverted - reverted block
    function revertBlock(Block memory _reverted) internal {
        require(
            _reverted.commitedAtBlock > 0,
            "rbkbnf"
        ); // rbkbnf - block not found
        require(
            _reverted.priorityOperations <= totalPriorityRequests,
            "rbkprc"
        ); // rbkprc - priority count too large in revert
        removeOnchainOps(_reverted.operationStartId, _reverted.onchainOperations);
    }

    // MARK: - Exodus mode

    // Check that current state not is exodus mode
    function requireActive() internal view {
        require(
            !exodusMode,
            "racexa"
        ); // racexa - exodus mode activated
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
        require(
            exodusMode,
            "extexm"
        ); // extexm - must be in exodus mode
        require(
            _owners.length == _amounts.length,
            "extaml"
        ); // extaml - |owners| != |amounts|

        for (uint256 i = 0; i < _owners.length; i++) {
            require(
                exited[_owners[i]][_tokenId] == false,
                "extaex"
            ); // extaex - already exited
        }

        // TODO: verify the proof that all users have the specified amounts of this token in the latest state

        for (uint256 i = 0; i < _owners.length; i++) {
            balancesToWithdraw[_owners[i]][_tokenId] += _amounts[i];
            exited[_owners[i]][_tokenId] = true;
        }
    }
}
