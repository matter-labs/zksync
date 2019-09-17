pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./VerificationKey.sol";
import "./Bytes.sol";

// GLOBAL TODOS:
// - check overflows

contract FranklinTest {
    // Verification key contract
    VerificationKey verificationKey;
    // Verifier contract
    Verifier verifier;
    // Governance contract
    Governance governance;

    // MARK: - CONSTANTS

    // Operation fields bytes lengths
    uint8 TOKEN_BYTES = 2; // token id
    uint8 AMOUNT_BYTES = 16; // token amount
    uint8 ETH_ADDR_BYTES = 20; // ethereum address
    uint8 FEE_BYTES = 2; // fee
    uint8 ACC_NUM_BYTES = 3; // franklin account id

    // Franklin chain address length
    uint8 constant PUBKEY_HASH_LEN = 20;
    // Signature (for example full exit signature) length
    uint8 constant SIGNATURE_LEN = 64;
    // Fee coefficient for priority request transaction
    uint256 constant FEE_COEFF = 4;
    // Base gas cost for transaction
    uint256 constant BASE_GAS = 21000;
    // Expiration delta for priority request to be satisfied (in ETH blocks)
    uint256 constant PRIORITY_EXPIRATION = 16;
    // Chunks per block; each chunk has 8 bytes of public data
    uint256 constant BLOCK_SIZE = 14;
    // Max amount of any token must fit into uint128
    uint256 constant MAX_VALUE = 2 ** 112 - 1;
    // ETH blocks verification expectation
    uint256 constant EXPECT_VERIFICATION_IN = 8;
    // Max number of unverified blocks. To make sure that all reverted blocks can be copied under block gas limit!
    uint256 constant MAX_UNVERIFIED_BLOCKS = 4;

    // Operations lengths

    uint256 constant NOOP_LENGTH = 1 * 8; // noop
    uint256 constant DEPOSIT_LENGTH = 6 * 8; // deposit
    uint256 constant TRANSFER_TO_NEW_LENGTH = 5 * 8; // transfer
    uint256 constant PARTIAL_EXIT_LENGTH = 6 * 8; // partial exit
    uint256 constant CLOSE_ACCOUNT_LENGTH = 1 * 8; // close account
    uint256 constant TRANSFER_LENGTH = 2 * 8; // transfer
    uint256 constant FULL_EXIT_LENGTH = 14 * 8; // full exit

    // MARK: - EVENTS

    // Event emitted when a block is committed
    // Structure:
    // - blockNumber - the number of committed block
    event BlockCommitted(uint32 indexed blockNumber);
    // Event emitted when a block is verified
    // Structure:
    // - blockNumber - the number of verified block
    event BlockVerified(uint32 indexed blockNumber);

    // Event emitted when blocks are reverted
    // Structure:
    // - totalBlocksVerified - number of verified blocks
    // - totalBlocksCommitted - number of committed blocks
    event BlocksReverted(
        uint32 indexed totalBlocksVerified,
        uint32 indexed totalBlocksCommitted
    );

    // Event emitted when user send a transaction to deposit her funds
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

    // Event emitted when user send a transaction to withdraw her funds from onchain balance
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
    // - opType - operation type
    // - pubData - operation data
    // - expirationBlock - the number of Ethereum block when request becomes expired
    // - fee - validators' fee
    event NewPriorityRequest(
        OpType indexed opType,
        bytes pubData,
        uint256 indexed expirationBlock,
        uint256 fee
    );

    // Exodus mode entered event
    event ExodusMode();

    // MARK: - STORAGE

    // Root-chain balances (per owner and token id) to withdraw
    mapping(address => mapping(uint16 => uint128)) public balancesToWithdraw;

    // Blocks

    // Total number of verified blocks
    // i.e. blocks[totalBlocksVerified] points at the latest verified block (block 0 is genesis)
    uint32 public totalBlocksVerified;

    // Total number of committed blocks
    // i.e. blocks[totalBlocksCommitted] points at the latest committed block
    uint32 public totalBlocksCommitted;

    // Block data (once per block)
    struct Block {
        // Validator (aka block producer)
        address validator;
        // ETH block number at which this block was committed
        uint32 committedAtBlock;
        // Index of the first operation to process for this block
        uint64 operationStartId;
        // Total number of operations to process for this block
        uint64 onchainOperations;
        // Total number of priority operations for this block
        uint64 priorityOperations;
        // Hash of commitment the block circuit
        bytes32 commitment;
        // New root hash
        bytes32 stateRoot;
    }

    // Blocks by Franklin block id
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

    // Onchain operation contains operation type and its committed data
    struct OnchainOperation {
        OpType opType;
        bytes pubData;
    }

    // Total number of registered onchain operations
    uint64 public totalOnchainOps;

    // Onchain operations by index
    mapping(uint64 => OnchainOperation) public onchainOps;

    // Priority Queue

    // Priority Operation contains operation type, its data, expiration block, and fee
    struct PriorityOperation {
        OpType opType;
        bytes pubData;
        uint256 expirationBlock;
        uint256 fee;
    }

    // Priority Requests mapping (request id - operation)
    // Contains op type, pubdata, fee and expiration block of unsatisfied requests.
    // Numbers are in order of requests receiving
    mapping(uint64 => PriorityOperation) public priorityRequests;
    // First priority request id
    uint64 public firstPriorityRequestId;
    // Total number of requests
    uint64 public totalOpenPriorityRequests;
    // Total number of committed requests
    uint64 public totalCommittedPriorityRequests;

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

    // MARK: - CONSTRUCTOR

    // Inits verifier, verification key and governance contracts instances,
    // sets genesis root
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

    // MARK: - PRIORITY QUEUE

    // Calculate expiration block for request, store this request and emit NewPriorityRequest event
    // Params:
    // - _opType - priority request type
    // - _fee - validators' fee
    // - _pubData - request data
    function addPriorityRequest(
        OpType _opType,
        uint256 _fee,
        bytes memory _pubData
    ) internal {
        // Expiration block is: current block number + priority expiration delta
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;

        priorityRequests[firstPriorityRequestId+totalOpenPriorityRequests] = PriorityOperation({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock,
            fee: _fee
        });
        totalOpenPriorityRequests++;

        emit NewPriorityRequest(
            _opType,
            _pubData,
            expirationBlock,
            _fee
        );
    }

    // Collects a fee from provided requests number for the validator, store it on her
    // balance to withdraw in Ether and delete this requests
    // Params:
    // - _number - the number of requests
    // - _validator - address to pay fee
    function collectValidatorsFeeAndDeleteRequests(uint64 _number, address _validator) internal {
        require(
            _number <= totalOpenPriorityRequests,
            "fcs11"
        ); // fcs11 - number is heigher than total priority requests number

        uint256 totalFee = 0;
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + _number; i++) {
            totalFee += priorityRequests[i].fee;
            delete priorityRequests[i];
        }
        totalOpenPriorityRequests -= _number;
        firstPriorityRequestId += _number;
        totalCommittedPriorityRequests -= _number;

        balancesToWithdraw[_validator][0] += uint128(totalFee);
    }

    // Accrues users balances from priority requests,
    // if this request contains a Deposit operation
    // WARNING: Only for Exodus mode
    function cancelOutstandingDepositsForExodusMode() internal {
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + totalOpenPriorityRequests; i++) {
            if (priorityRequests[i].opType == OpType.Deposit) {
                bytes memory pubData = priorityRequests[i].pubData;
                bytes memory owner = new bytes(ETH_ADDR_BYTES);
                for (uint8 j = 0; j < ETH_ADDR_BYTES; ++j) {
                    owner[j] = pubData[j];
                }
                bytes memory token = new bytes(TOKEN_BYTES);
                for (uint8 j = 0; j < TOKEN_BYTES; j++) {
                    token[j] = pubData[ETH_ADDR_BYTES + j];
                }
                bytes memory amount = new bytes(AMOUNT_BYTES);
                for (uint8 j = 0; j < AMOUNT_BYTES; ++j) {
                    amount[j] = pubData[ETH_ADDR_BYTES + TOKEN_BYTES + j];
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

    // MARK: - ROOT-CHAIN OPERATIONS

    // Deposit ETH
    // Params:
    // - _franklinAddr - the receiver Franklin address
    function depositETH(bytes calldata _franklinAddr) external payable {
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee,
            "fdh11"
        ); // fdh11 - Not enough ETH provided to pay the fee
        
        uint256 amount = msg.value-fee;
        require(
            amount <= MAX_VALUE,
            "fdh12"
        ); // fdh12 - deposit amount value is heigher than Franklin is able to process

        registerDeposit(0, uint128(amount), fee, _franklinAddr);
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
    ) external payable {
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee,
            "fd011"
        ); // fd011 - Not enough ETH provided to pay the fee

        require(
            IERC20(_token).transferFrom(msg.sender, address(this), _amount),
            "fd012"
        ); // fd012 - token transfer failed deposit

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(_token);
        
        registerDeposit(tokenId, _amount, fee, _franklinAddr);

        if (msg.value != fee) {
            msg.sender.transfer(msg.value-fee);
        }
    }

    // Withdraw ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external {
        uint16 tokenId = governance.validateTokenAddress(_token);
        registerWithdrawal(tokenId, _amount);
        require(
            IERC20(_token).transfer(msg.sender, _amount),
            "fw011"
        ); // fw011 - token transfer failed withdraw
    }
    
    // Register full exit request
    // Params:
    // - _franklinId - sender
    // - _token - token address, 0 address for ether
    // - _signature - user signature
    function fullExit (
        uint24 _franklinId,
        address _token,
        bytes calldata _signature
    ) external payable {
        // Fee is:
        //   fee coeff * (base tx gas cost + remained gas) * gas price
        uint256 fee = FEE_COEFF * (BASE_GAS + gasleft()) * tx.gasprice;
        
        uint16 tokenId;
        if (_token == address(0)) {
            tokenId = 0;
        } else {
            tokenId = governance.validateTokenAddress(_token);
        }
        
        require(
            msg.value >= fee,
            "fft11"
        ); // fft11 - Not enough ETH provided to pay the fee
        
        requireActive();

        require(
            _signature.length == SIGNATURE_LEN,
            "fft12"
        ); // fft12 - wrong signature length

        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromUInt24(_franklinId); // franklin id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(msg.sender)); // eth address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId)); // token id
        pubData = Bytes.concat(pubData, _signature); // signature
        addPriorityRequest(OpType.FullExit, fee, pubData);
        
        if (msg.value != fee) {
            msg.sender.transfer(msg.value-fee);
        }
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
        require(
            _franklinAddr.length == PUBKEY_HASH_LEN,
            "frd11"
        ); // frd11 - wrong franklin address hash
        
        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromAddress(msg.sender); // sender
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(_token)); // token id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt128(_amount)); // amount
        pubData = Bytes.concat(pubData, _franklinAddr); // franklin address
        addPriorityRequest(OpType.Deposit, _fee, pubData);

        emit OnchainDeposit(
            msg.sender,
            _token,
            _amount,
            _franklinAddr
        );
    }

    // Register withdrawal
    // Params:
    // - _token - token by id
    // - _amount - token amount
    function registerWithdrawal(uint16 _token, uint128 _amount) internal {
        require(
            balancesToWithdraw[msg.sender][_token] >= _amount,
            "frw11"
        ); // frw11 - insufficient balance withdraw

        balancesToWithdraw[msg.sender][_token] -= _amount;

        emit OnchainWithdrawal(
            msg.sender,
            _token,
            _amount
        );
    }

    // MARK: - BLOCK COMMITMENT

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
            _blockNumber == totalBlocksCommitted + 1,
            "fck11"
        ); // fck11 - only commit next block
        require(
            totalBlocksCommitted - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS,
            "fck12"
        ); // fck12 - too many committed
        require(
            governance.isValidator(msg.sender),
            "fck13"
        ); // fck13 - not a validator in commit
        if(!triggerRevertIfBlockCommitmentExpired() && !triggerExodusIfNeeded()) {
            // Unpack onchain operations and store them.
            // Get onchain operations start id for global onchain operations counter,
            // onchain operations number for this block, priority operations number for this block.
            (uint64 startId, uint64 totalProcessed, uint64 priorityNumber) = collectOnchainOps(_publicData);

            // Verify that priority operations from this block are valid
            // (their data is similar to data from priority requests mapping)
            verifyPriorityOperations(startId, totalProcessed, priorityNumber);

            // Create block commitment for verification proof
            bytes32 commitment = createBlockCommitment(
                _blockNumber,
                _feeAccount,
                blocks[_blockNumber - 1].stateRoot,
                _newRoot,
                _publicData
            );

            blocks[_blockNumber] = Block(
                msg.sender, // validator
                uint32(block.number), // committed at
                startId, // blocks' onchain ops start id in global operations
                totalProcessed, // total number of onchain ops in block
                priorityNumber, // total number of priority onchain ops in block
                commitment, // blocks' commitment
                _newRoot // new root
            );

            totalOnchainOps = startId + totalProcessed;

            totalBlocksCommitted += 1;

            totalCommittedPriorityRequests += priorityNumber;

            emit BlockCommitted(_blockNumber);
        }
    }

    // Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    // Returns onchain operations start id for global onchain operations counter,
    // onchain operations number for this block, priority operations number for this block
    // Params:
    // - _publicData - operations packed in bytes array
    function collectOnchainOps(bytes memory _publicData)
        internal
        returns (uint64 onchainOpsStartId, uint64 processedOnchainOps, uint64 priorityCount)
    {
        require(
            _publicData.length % 8 == 0,
            "fcs21"
        ); // fcs21 - pubdata.len % 8 != 0

        onchainOpsStartId = totalOnchainOps;
        uint64 currentOnchainOp = totalOnchainOps;

        uint256 currentPointer = 0;

        while (currentPointer < _publicData.length) {
            uint8 opType = uint8(_publicData[currentPointer]);
            (uint256 len, uint64 ops, uint64 priority) = processOp(
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
            "fcs22"
        ); // fcs22 - last chunk exceeds pubdata
    }

    // Returns operation processed length, and indicators if it is
    // an onchain operation and if it is a priority operation (1 if true)
    // Params:
    // - _opType - operation type
    // - _currentPointer - current pointer
    // - _publicData - operation data
    // - _currentOnchainOp - operation identifier
    function processOp(
        uint8 _opType,
        uint256 _currentPointer,
        bytes memory _publicData,
        uint64 _currentOnchainOp
    ) internal returns (uint256 processedLen, uint64 processedOnchainOps, uint64 priorityCount) {
        uint256 opDataPointer = _currentPointer + 1;

        if (_opType == uint8(OpType.Noop)) return (NOOP_LENGTH, 0, 0);
        if (_opType == uint8(OpType.TransferToNew)) return (TRANSFER_TO_NEW_LENGTH, 0, 0);
        if (_opType == uint8(OpType.Transfer)) return (TRANSFER_LENGTH, 0, 0);
        if (_opType == uint8(OpType.CloseAccount)) return (CLOSE_ACCOUNT_LENGTH, 0, 0);

        if (_opType == uint8(OpType.Deposit)) {
            bytes memory pubData = Bytes.slice(_publicData, opDataPointer + ACC_NUM_BYTES, TOKEN_BYTES + AMOUNT_BYTES + PUBKEY_HASH_LEN);
            require(
                pubData.length == TOKEN_BYTES + AMOUNT_BYTES + PUBKEY_HASH_LEN,
                "fpp11"
            ); // fpp11 - wrong deposit length
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.Deposit,
                pubData
            );
            return (DEPOSIT_LENGTH, 1, 1);
        }

        if (_opType == uint8(OpType.PartialExit)) {
            bytes memory pubData = Bytes.slice(_publicData, opDataPointer + ACC_NUM_BYTES, TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + ETH_ADDR_BYTES);
            require(
                pubData.length == TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + ETH_ADDR_BYTES,
                "fpp12"
            ); // fpp12 - wrong partial exit length
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.PartialExit,
                pubData
            );
            return (PARTIAL_EXIT_LENGTH, 1, 0);
        }

        if (_opType == uint8(OpType.FullExit)) {
            bytes memory pubData = Bytes.slice(_publicData, opDataPointer, ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + SIGNATURE_LEN + AMOUNT_BYTES);
            require(
                pubData.length == ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + SIGNATURE_LEN + AMOUNT_BYTES,
                "fpp13"
            ); // fpp13 - wrong full exit length
            onchainOps[_currentOnchainOp] = OnchainOperation(
                OpType.FullExit,
                pubData
            );
            return (FULL_EXIT_LENGTH, 1, 1);
        }

        revert("fpp14"); // fpp14 - unsupported op
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
        // public data is committed with padding (TODO: check assembly and optimize to avoid copying data)
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
    // - _number - priority ops number
    function verifyPriorityOperations(uint64 _startId, uint64 _totalProcessed, uint64 _number) internal view {
        require(
            _number <= totalOpenPriorityRequests-totalCommittedPriorityRequests,
            "fvs11"
        ); // fvs11 - too much priority requests
        
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;
        
        uint64 counter = 0;
        for (uint64 current = start; current < end; ++current) {
            if (onchainOps[current].opType == OpType.FullExit || onchainOps[current].opType == OpType.Deposit) {
                OnchainOperation memory op = onchainOps[current];
                require(
                    isPriorityOpValid(op, counter+firstPriorityRequestId+totalCommittedPriorityRequests),
                    "fvs12"
                ); // fvs12 - priority operation is not valid
                counter++;
            }
        }
    }

    // Compares operation from the block with corresponding priority requests' operation
    // Params:
    // - _onchainOp - operation from block
    // - _priorityRequestId - priority request id
    function isPriorityOpValid(OnchainOperation memory _onchainOp, uint64 _priorityRequestId) internal view returns (bool) {
        bytes memory priorityPubData;
        bytes memory onchainPubData;
        if (_onchainOp.opType == OpType.Deposit && priorityRequests[_priorityRequestId].opType == OpType.Deposit) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, ETH_ADDR_BYTES, PUBKEY_HASH_LEN + AMOUNT_BYTES + TOKEN_BYTES);
            onchainPubData = _onchainOp.pubData;
        } else if (_onchainOp.opType == OpType.FullExit && priorityRequests[_priorityRequestId].opType == OpType.FullExit) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, 0, ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + SIGNATURE_LEN);
            onchainPubData = Bytes.slice(_onchainOp.pubData, 0, ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + SIGNATURE_LEN);
        } else {
            revert("fid11"); // fid11 - wrong operation
        }
        return (priorityPubData.length > 0) &&
            (keccak256(onchainPubData) == keccak256(priorityPubData));
    }

    // Removes some onchain ops (for example in case of wrong priority comparison)
    // Params:
    // - _startId - onchain op start id
    // - _totalProcessed - how many ops are procceeded
    function revertOnchainOps(uint64 _startId, uint64 _totalProcessed) internal {
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;

        for (uint64 current = start; current < end; ++current) {
            delete onchainOps[current];
        }
    }

    // MARK: - BLOCK VERIFICATION

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
            "fvk11"
        ); // fvk11 - only verify next block
        require(
            governance.isValidator(msg.sender),
            "fvk12"
        ); // fvk12 - not a validator in verify

        // TODO: - doesnt work in integration test - revert with vfyfp3 code. Need to be fixed
        // require(
        //     verifyBlockProof(_proof, blocks[_blockNumber].commitment),
        //     "fvk13"
        // ); // fvk13 - verification failed

        consummateOnchainOps(_blockNumber);

        collectValidatorsFeeAndDeleteRequests(
            blocks[_blockNumber].priorityOperations,
            blocks[_blockNumber].validator
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

    // If block is verified the onchain operations from it must be completed
    // (user must have possibility to withdraw funds if withdrawed)
    // Params:
    // - _blockNumber - number of block
    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 start = blocks[_blockNumber].operationStartId;
        uint64 end = start + blocks[_blockNumber].onchainOperations;
        for (uint64 current = start; current < end; ++current) {
            OnchainOperation memory op = onchainOps[current];
            if (op.opType == OpType.PartialExit) {
                // partial exit was successful, accrue balance
                bytes memory tokenBytes = new bytes(TOKEN_BYTES);
                for (uint8 i = 0; i < TOKEN_BYTES; ++i) {
                    tokenBytes[i] = op.pubData[i];
                }
                uint16 tokenId = Bytes.bytesToUInt16(tokenBytes);

                bytes memory amountBytes = new bytes(AMOUNT_BYTES);
                for (uint256 i = 0; i < AMOUNT_BYTES; ++i) {
                    amountBytes[i] = op.pubData[TOKEN_BYTES + i];
                }
                uint128 amount = Bytes.bytesToUInt128(amountBytes);

                bytes memory ethAddress = new bytes(ETH_ADDR_BYTES);
                for (uint256 i = 0; i < ETH_ADDR_BYTES; ++i) {
                    ethAddress[i] = op.pubData[TOKEN_BYTES + AMOUNT_BYTES + FEE_BYTES + i];
                }
                balancesToWithdraw[Bytes.bytesToAddress(ethAddress)][tokenId] += amount;
            }
            if (op.opType == OpType.FullExit) {
                // full exit was successful, accrue balance
                bytes memory tokenBytes = new bytes(TOKEN_BYTES);
                for (uint8 i = 0; i < TOKEN_BYTES; ++i) {
                    tokenBytes[i] = op.pubData[ACC_NUM_BYTES + ETH_ADDR_BYTES + i];
                }
                uint16 tokenId = Bytes.bytesToUInt16(tokenBytes);

                bytes memory amountBytes = new bytes(AMOUNT_BYTES);
                for (uint256 i = 0; i < AMOUNT_BYTES; ++i) {
                    amountBytes[i] = op.pubData[ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES + SIGNATURE_LEN + i];
                }
                uint128 amount = Bytes.bytesToUInt128(amountBytes);

                bytes memory ethAddress = new bytes(ETH_ADDR_BYTES);
                for (uint256 i = 0; i < ETH_ADDR_BYTES; ++i) {
                    ethAddress[i] = op.pubData[ACC_NUM_BYTES + i];
                }
                balancesToWithdraw[Bytes.bytesToAddress(ethAddress)][tokenId] += amount;
            }
            delete onchainOps[current];
        }
    }

    // MARK: - REVERTING COMMITTED BLOCKS

    // Checks that commitment is expired and revert blocks
    function triggerRevertIfBlockCommitmentExpired() internal returns (bool) {
        if (totalBlocksCommitted > totalBlocksVerified &&
                block.number >
                blocks[totalBlocksVerified + 1].committedAtBlock +
                    EXPECT_VERIFICATION_IN) {
            revertBlocks();
            return true;
        }
        return false;
    }

    // Reverts unverified blocks
    function revertBlocks() internal {
        for (uint32 i = totalBlocksVerified; i < totalBlocksCommitted-1; i++) {
            Block memory reverted = blocks[i];
            revertBlock(reverted);
            delete blocks[i];
        }
        totalBlocksCommitted -= totalBlocksCommitted - totalBlocksVerified;
        emit BlocksReverted(totalBlocksVerified, totalBlocksCommitted);
    }

    // Reverts block onchain operations
    // Params:
    // - _reverted - reverted block
    function revertBlock(Block memory _reverted) internal {
        require(
            _reverted.committedAtBlock > 0,
            "frk11"
        ); // frk11 - block not found
        revertOnchainOps(_reverted.operationStartId, _reverted.onchainOperations);
        totalCommittedPriorityRequests -= _reverted.priorityOperations;
    }

    // MARK: - EXODUS MODE

    // Checks that current state not is exodus mode
    function requireActive() internal view {
        require(
            !exodusMode,
            "fre11"
        ); // fre11 - exodus mode activated
    }

    // Checks if Exodus mode must be entered. If true - cancels outstanding deposits and emits ExodusMode event.
    // Returns bool flag that is true if the Exodus mode must be entered.
    // Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    // of existed priority requests expiration block number.
    function triggerExodusIfNeeded() internal returns (bool) {
        if (block.number >= priorityRequests[firstPriorityRequestId].expirationBlock) {
            exodusMode = true;
            cancelOutstandingDepositsForExodusMode();
            emit ExodusMode();
            return true;
        } else {
            return false;
        }
    }

    // Withdraws token from Franklin to root chain
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
            "fet11"
        ); // fet11 - must be in exodus mode
        require(
            _owners.length == _amounts.length,
            "fet12"
        ); // fet12 - |owners| != |amounts|

        for (uint256 i = 0; i < _owners.length; i++) {
            require(
                exited[_owners[i]][_tokenId] == false,
                "fet13"
            ); // fet13 - already exited
        }

        // TODO: verify the proof that all users have the specified amounts of this token in the latest state

        for (uint256 i = 0; i < _owners.length; i++) {
            balancesToWithdraw[_owners[i]][_tokenId] += _amounts[i];
            exited[_owners[i]][_tokenId] = true;
        }
    }
}