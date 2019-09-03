pragma solidity ^0.5.1;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Verifier.sol";
import "./VerificationKey.sol";
import "./PriorityQueue.sol";
import "./Bytes.sol";

contract Franklin {
    VerificationKey verificationKey;
    Verifier verifier;

    // chunks per block; each chunk has 8 bytes of public data
    uint256 constant BLOCK_SIZE = 10;
    // must fit into uint112
    uint256 constant MAX_VALUE = 2 ** 112 - 1;
    // ETH blocks
    uint256 constant EXPECT_VERIFICATION_IN = 8 * 60 * 100;
    // To make sure that all reverted blocks can be copied under block gas limit!
    uint256 constant MAX_UNVERIFIED_BLOCKS = 4 * 60 * 100;
    // Offchain address length.
    uint8 constant PUBKEY_HASH_LEN = 20;

    // Possible token
    struct ValidatedTokenId {
        uint32 id;
    }

    // MARK: - Events

    // Event emitted when block is commited
    // Structure:
    // - blockNumber - number of commited block
    event Blockcommited(uint32 indexed blockNumber);
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
        uint32 tokenId,
        uint112 amount,
        bytes franklinAddress
    );

    // Event emitted when user send withdraw transaction from this contract
    // Structure:
    // - owner - sender
    // - tokenId - withdrawed token
    // - amount - withdrawed value
    event OnchainWithdrawal(
        address indexed owner,
        uint32 tokenId,
        uint112 amount
    );

    // Token added to Franklin net
    // Structure:
    // - token - added token address
    // - tokenId - added token id
    event TokenAdded(
        address token,
        uint32 tokenId
    );

    // New priority request event
    // Emitted when a request is placed into mapping
    // Params:
    // - opType - request type
    // - pubData - request data
    // - expirationBlock - the number of Ethereum block when request becomes expired
    event NewPriorityRequest(
        uint indexed opType,
        bytes pubData,
        uint indexed expirationBlock
    );

    // MARK: - STORAGE

    // Governance

    // Address which will excercise governance over the network
    // i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    // Total number of ERC20 tokens registered in the network
    // (excluding ETH, which is hardcoded as tokenId = 0)
    uint32 public totalTokens;

    // List of registered tokens by tokenId
    mapping(uint32 => address) public tokenAddresses;

    // List of registered tokens by address
    mapping(address => uint32) public tokenIds;

    // List of permitted validators
    mapping(address => bool) public validators;

    // List of root-chain balances (per owner and tokenId) to withdraw
    mapping(address => mapping(uint32 => uint112)) public balancesToWithdraw;

    // TODO: - fix
    // uint32 public totalAccounts;
    // mapping (address => uint32) public accountIdByAddress;
    ///////////////

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
        uint priorityOperations;
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
        uint32 tokenId;
        address owner;
        uint112 amount;
    }

    // Total number of registered OnchainOps
    uint64 totalOnchainOps;

    // List of OnchainOps by index
    mapping(uint64 => OnchainOp) public onchainOps;

    // Priority Queue

    // Priority request op type and expiration block
    struct PriorityRequestParams {
        uint opType;
        uint expirationBlock;
    }

    // Requests params mapping (request id - (type, expiration block))
    // Contains op type and expiration block of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint => PriorityRequestParams) public priorityRequestsParams;
    // First priority request id
    uint public firstPriorityRequestId;
    // Total number of requests
    uint public totalPriorityRequests;

    // Reverting expired blocks

    // Total number of registered blocks to revert (see docs)
    uint32 totalBlocksToRevert;

    // List of blocks by revertBlockId (see docs)
    mapping(uint32 => Block) public blocksToRevert;

    // Flag indicating that exodus (mass exit) mode is triggered
    // Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

    // Flag indicating that a user has exited certain token balance (per owner and tokenId)
    mapping(address => mapping(uint32 => bool)) public exited;

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
        address _networkGovernor
    ) public {
        verifier = Verifier(_verifierAddress);
        verificationKey = VerificationKey(_vkAddress);

        blocks[0].stateRoot = _genesisRoot;
        networkGovernor = _networkGovernor;

        // TODO: remove once proper governance is implemented
        validators[_networkGovernor] = true;
    }

    // MARK: - Priority Queue

    // Add request
    // Params:
    // - _opType - priority request type
    // - _pubData - request data
    function addPriorityRequest(uint _opType, bytes calldata _pubData) internal {
        uint expirationBlock = block.number + EXPECT_VERIFICATION_IN;
        priorityRequestsParams[totalPriorityRequests] = PriorityRequestParams({
            opType: _opType,
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
    function removePriorityRequests(uint _count) internal {
        require(count <= totalRequests, "Count of removed requests is higher than their count");

        for (uint i = firstPriorityRequestId; i < firstPriorityRequestId+count; i++) {
            delete priorityRequestsParams[i];
        }
        totalPriorityRequests -= count;
        firstPriorityRequestId += count;
    }

    // Removes certain requests
    // Params:
    // - _opType - operation type
    // - _anyRequestsCount - count of requests where to look certain requests for
    function removePriorityRequestsWithType(uint _opType, uint _anyRequestsCount) internal {
        require(_anyRequestsCount <= totalRequests, "Count of removed requests is higher than their count");

        uint removingPriorityCount = 0;
        for (uint i = firstPriorityRequestId; i < firstPriorityRequestId + _anyRequestsCount; i++) {
            if (priorityRequestsParams[i].opType == _opType) {
                delete priorityRequestsParams[i];
                removingPriorityCount++;
            }
        }
        totalPriorityRequests -= removingPriorityCount;
        firstPriorityRequestId += removingPriorityCount;
    }

    // MARK: - Governance

    // Change current governor
    // _newGovernor - address of the new governor
    function changeGovernor(address _newGovernor) external {
        requireGovernor();
        networkGovernor = _newGovernor;
    }

    // Add token to the list of possible tokens
    // Params:
    // - _token - token address
    function addToken(address _token) external {
        requireGovernor();
        require(tokenIds[_token] == 0, "token exists");
        tokenAddresses[totalTokens + 1] = _token; // Adding one because tokenId = 0 is reserved for ETH
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

    // Set validator status
    // Params:
    // - _validator - validator address
    // - _active - bool value (true if validator is active)
    function setValidator(address _validator, bool _active) external {
        requireGovernor();
        validators[_validator] = _active;
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
    // function migrateToken(uint32 _tokenId, uint112 /*_amount*/, bytes calldata /*_proof*/) external {
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
        registerDeposit(ValidatedTokenId(0), uint112(msg.value), _franklinAddr);
    }

    // Withdraw ETH
    // Params:
    // - _amount - amount to withdraw
    function withdrawETH(uint112 _amount) external {
        registerWithdrawal(ValidatedTokenId(0), _amount);
        msg.sender.transfer(_amount);
    }

    // Deposit ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount of token
    // - _franklin_addr - receiver
    function depositERC20(
        address _token,
        uint112 _amount,
        bytes calldata _franklin_addr
    ) external {
        require(
            IERC20(_token).transferFrom(msg.sender, address(this), _amount),
            "transfer failed"
        );
        ValidatedTokenId memory tokenId = validateERC20Token(_token);
        registerDeposit(tokenId, _amount, _franklin_addr);
    }

    // Withdraw ERC20 token
    // Params:
    // - _token - token address
    // - _amount - amount to withdraw
    function withdrawERC20(address _token, uint112 _amount) external {
        ValidatedTokenId memory tokenId = validateERC20Token(_token);
        registerWithdrawal(tokenId, _amount);
        require(
            IERC20(_token).transfer(msg.sender, _amount),
            "transfer failed"
        );
    }

    // Register full exit request
    // Params:
    // - _franklin_addr - sender
    // - _eth_addr - receiver
    // - _token - token address
    // - _signature - user signature
    function registerFullExit(
        bytes memory _franklin_addr,
        address _eth_addr,
        address _token,
        bytes20 signature
    ) external {
        requireActive();
        ValidatedTokenId memory tokenId = validateERC20Token(_token);
        // Priority Queue request
        bytes memory pubData = Bytes.concat(_franklin_addr); // franklin address
        pubData = Bytes.concat(Bytes.toBytesFromAddress(_eth_addr)); // eth address
        pubData = Bytes.concat(Bytes.toBytesFromUInt32(tokenId.id)); // token id
        pubData = Bytes.concat(bytes(signature)); // signature
        addPriorityRequest(OpType.FullExit, pubData);
    }

    // Register deposit request
    // Params:
    // - _token - token by id
    // - _amount - token amount
    // - _franklin_addr - receiver
    function registerDeposit(
        ValidatedTokenId memory _token,
        uint112 _amount,
        bytes memory _franklin_addr
    ) internal {
        requireActive();

        emit OnchainDeposit(
            msg.sender,
            _token.id,
            _amount,
            _franklin_addr
        );

        // Priority Queue request
        bytes memory pubData = Bytes.concat(Bytes.toBytesFromAddress(msg.sender)); // sender
        pubData = Bytes.concat(Bytes.toBytesFromUInt32(_token.id)); // token id
        pubData = Bytes.concat(Bytes.toBytesFromUInt112(_amount)); // amount
        pubData = Bytes.concat(_franklin_addr); // franklin address
        addPriorityRequest(OpType.Deposit, pubData);
    }

    // Register withdrawal
    // Params:
    // - _token - token by id
    // - _amount - token amount
    function registerWithdrawal(ValidatedTokenId memory _token, uint112 _amount) internal {
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
        require(validators[msg.sender], "only by validator");
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

        // TODO: make efficient padding here

        (uint64 startId, uint64 totalProcessed, uint priorityOperations) = commitOnchainOps(_publicData);

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
        emit Blockcommited(_blockNumber);
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
        returns (uint64 onchainOpsStartId, uint64 processedOnchainOps, uint priorityOperations)
    {
        require(_publicData.length % 8 == 0, "pubdata.len % 8 != 0");

        onchainOpsStartId = totalOnchainOps;
        uint64 currentOnchainOp = totalOnchainOps;

        // NOTE: the stuff below is the most expensive and most frequently used part of the entire contract.
        // It is highly unoptimized and can be improved by an order of magnitude by getting rid of the subroutine,
        // using assembly, replacing ifs with mem lookups and other tricks
        // TODO: optimize
        uint256 currentPointer = 0;

        uint priorityOperations = priorityOps;
        while (currentPointer < _publicData.length) {
            uint opType = uint(bytes1(_publicData[currentPointer]));
            (uint256 len, uint64 ops, uint priorityOps) = processOp(
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
        uint _opType,
        uint256 _currentPointer,
        bytes memory _publicData,
        uint64 _currentOnchainOp
    ) internal returns (uint256 processedLen, uint64 processedOnchainOps, uint priorityOperations) {
        uint256 opDataPointer = _currentPointer + 1;

        if (_opType == OpType.Noop) return (1 * 8, 0, 0); // noop
        if (_opType == OpType.TransferToNew) return (5 * 8, 0, 0); // transfer_to_new
        if (_opType == OpType.Transfer) return (2 * 8, 0, 0); // transfer
        if (_opType == OpType.CloseAccount) return (1 * 8, 0, 0); // close_account

        // deposit
        if (_opType == OpType.Deposit) {
            // to_account: 3, token: 2, amount: 3, fee: 1, new_pubkey_hash: 20

            uint16 tokenId = uint16(
                (uint256(uint8(_publicData[opDataPointer + 3])) << 8) +
                    uint256(uint8(_publicData[opDataPointer + 4]))
            );

            uint8[3] memory amountPacked;
            amountPacked[0] = uint8(_publicData[opDataPointer + 5]);
            amountPacked[1] = uint8(_publicData[opDataPointer + 6]);
            amountPacked[2] = uint8(_publicData[opDataPointer + 7]);
            uint112 amount = unpackAmount(amountPacked);

            uint8 feePacked = uint8(_publicData[opDataPointer + 8]);
            uint112 fee = unpackFee(feePacked);

            bytes memory franklin_address_ = new bytes(PUBKEY_HASH_LEN);
            for (uint8 i = 0; i < PUBKEY_HASH_LEN; i++) {
                franklin_address_[i] = _publicData[opDataPointer + 9 + i];
            }
            address account = depositFranklinToETH[franklin_address_];

            requireValidTokenId(tokenId);

            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Deposit,
                tokenId,
                account,
                (amount + fee)
            );
            return (5 * 8, 1, 1);
        }

        // partial_exit
        if (_opType == OpType.PartialExit) {
            // pubdata account: 3, token: 2, amount: 3, fee: 1, eth_key: 20

            uint16 tokenId = uint16(
                (uint256(uint8(_publicData[opDataPointer + 3])) << 8) +
                    uint256(uint8(_publicData[opDataPointer + 4]))
            );

            uint8[3] memory amountPacked;
            amountPacked[0] = uint8(_publicData[opDataPointer + 5]);
            amountPacked[1] = uint8(_publicData[opDataPointer + 6]);
            amountPacked[2] = uint8(_publicData[opDataPointer + 7]);
            uint112 amount = unpackAmount(amountPacked);

            bytes memory ethAddress = new bytes(20);
            for (uint256 i = 0; i < 20; ++i) {
                ethAddress[i] = _publicData[opDataPointer + 9 + i];
            }

            requireValidTokenId(tokenId);
            // TODO!: balances[ethAddress][tokenId] possible overflow (uint112)
            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Withdrawal,
                tokenId,
                Bytes.bytesToAddress(ethAddress),
                amount
            );
            return (4 * 8, 1, 0);
        }

        // full_exit
        if (_opType == OpType.FullExit) {
            // pubdata account: 3, eth_address: 20, token: 2, signature_hash: 20, full_amount: 14

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

            bytes memory fullAmountBytes = new bytes(14);
            for (uint256 i = 0; i < 14; ++i) {
                fullAmountBytes[i] = _publicData[opDataPointer + 45 + i];
            }
            uint112 fullAmount = Bytes.bytesToUint112(fullAmountBytes);

            requireValidTokenId(tokenId);
            // TODO!: balances[ethAddress][tokenId] possible overflow (uint112)
            onchainOps[_currentOnchainOp] = OnchainOp(
                OnchainOpType.Withdrawal,
                tokenId,
                Bytes.bytesToAddress(ethAddress),
                fullAmount
            );
            return (7 * 8, 1, 1);
        }

        require(false, "unsupported op");
    }

    // MARK: - Block verification

    // Block verification
    // Params:
    // - blockNumber - block number
    // - proof - proof
    function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof)
        external
    {
        requireActive();
        require(validators[msg.sender], "only by validator");
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
    function revertExpiredBlocks() external {
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

    // Revert block onchain operations and remove deposit priority requests from its mapping
    // Params:
    // - _revertedBlockId - block id
    function revertBlock(uint32 _revertedBlockId) external {
        Block memory reverted = blocksToRevert[_revertedBlockId];
        require(reverted.commitedAtBlock > 0, "block not found");
        require(reverted.priorityOperations <= totalRequests, "Count of removed requests is higher than their count");

        uint64 current = reverted.operationStartId;
        uint64 end = current + reverted.totalOperations;
        while (current < end) {
            OnchainOp memory op = onchainOps[current];
            if (op.opType == OnchainOpType.Deposit) {
                // deposit failed, return funds
                balancesToWithdraw[op.owner][op.tokenId] += op.amount;
            }
            delete onchainOps[current];
        }

        // delete deposits in priority requests
        removePriorityRequestsWithType(OpType.Deposit, reverted.priorityOperations);

        delete blocksToRevert[_revertedBlockId];
    }

    // MARK: - Exodus mode

    // Returns bool flag. True if the Exodus mode must be entered
    function triggerExodusIfNeeded() internal view returns (bool) {
        if (block.number >= priorityRequestsParams[firstPriorityRequestId]) {
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
        uint32 _tokenId,
        address[] calldata _owners,
        uint112[] calldata _amounts,
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

    function requireGovernor() internal view {
        require(msg.sender == networkGovernor, "only by governor");
    }

    function requireActive() internal view {
        require(!exodusMode, "exodus mode");
    }

    function requireValidTokenId(uint32 _tokenId) internal view {
        require(_tokenId < totalTokens + 1, "unknown token");
    }

    function validateERC20Token(address tokenAddr) internal view returns (ValidatedTokenId memory) {
        uint32 tokenId = tokenIds[tokenAddr];
        require(tokenAddresses[tokenId] == tokenAddr, "unknown ERC20 token");
        return ValidatedTokenId(tokenId);
    }

    function blockCommitmentExpired() internal view returns (bool) {
        return
            totalBlocksCommited > totalBlocksVerified &&
                block.number >
                blocks[totalBlocksVerified + 1].commitedAtBlock +
                    EXPECT_VERIFICATION_IN;
    }

    function unpackAmount(uint8[3] memory _amount)
        internal
        pure
        returns (uint112)
    {
        uint24 n = (uint24(_amount[0]) << 2*8)
        + (uint24(_amount[1]) << 8)
        + (uint24(_amount[2]));
        return uint112(n >> 5) * (uint112(10) ** (n & 0x1f));
    }


    function unpackFee(uint8 encoded_fee) internal pure returns (uint112) {
        return uint112(encoded_fee >> 4) * uint112(10) ** (encoded_fee & 0x0f);
    }

}