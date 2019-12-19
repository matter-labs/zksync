pragma solidity 0.5.10;

import "../node_modules/openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./PriorityQueue.sol";
import "./Bytes.sol";

// GLOBAL TODOS:
// - check overflows

contract Franklin {
    // Verifier contract
    Verifier internal verifier;
    // Governance contract
    Governance internal governance;
    // Priority Queue contract
    PriorityQueue internal priorityQueue;

    // Operation fields bytes lengths
    uint8 constant TOKEN_BYTES = 2; // token id
    uint8 constant AMOUNT_BYTES = 16; // token amount
    uint8 constant ETH_ADDR_BYTES = 20; // ethereum address
    uint8 constant FEE_BYTES = 2; // fee
    uint8 constant ACC_NUM_BYTES = 3; // franklin account id
    uint8 constant NONCE_BYTES = 4; // franklin nonce

    // Franklin chain address length
    uint8 constant PUBKEY_HASH_LEN = 20;
    // Signature (for example full exit signature) length
    uint8 constant SIGNATURE_LEN = 64;
    // Public key length
    uint8 constant PUBKEY_LEN = 32;
    // Fee coefficient for priority request transaction
    uint256 constant FEE_COEFF = 2;
    // Base gas cost for deposit eth transaction
    uint256 constant BASE_DEPOSIT_ETH_GAS = 179000;
    // Base gas cost for deposit erc transaction
    uint256 constant BASE_DEPOSIT_ERC_GAS = 214000;
    // Base gas cost for full exit transaction
    uint256 constant BASE_FULL_EXIT_GAS = 170000;
    // Base gas cost for transaction
    uint256 constant BASE_GAS = 21000;
    // Max amount of any token must fit into uint128
    uint256 constant MAX_VALUE = 2 ** 112 - 1;
    // ETH blocks verification expectation
    uint256 constant EXPECT_VERIFICATION_IN = 8 * 60 * 100;
    // Max number of unverified blocks. To make sure that all reverted blocks can be copied under block gas limit!
    uint256 constant MAX_UNVERIFIED_BLOCKS = 4 * 60 * 100;

    // Operations lengths
    uint256 constant NOOP_LENGTH = 1 * 8; // noop
    uint256 constant DEPOSIT_LENGTH = 6 * 8; // deposit
    uint256 constant TRANSFER_TO_NEW_LENGTH = 5 * 8; // transfer
    uint256 constant PARTIAL_EXIT_LENGTH = 6 * 8; // partial exit
    uint256 constant CLOSE_ACCOUNT_LENGTH = 1 * 8; // close account
    uint256 constant TRANSFER_LENGTH = 2 * 8; // transfer
    uint256 constant FULL_EXIT_LENGTH = 18 * 8; // full exit


    // Event emitted when a block is committed
    // Structure:
    // - blockNumber - the number of committed block
    event BlockCommitted(uint32 indexed blockNumber);
    // Event emitted when a block is verified
    // Structure:
    // - blockNumber - the number of verified block
    event BlockVerified(uint32 indexed blockNumber);

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

    // Event emitted when user send a transaction to deposit her funds
    // Structure:
    // - owner - sender
    // - tokenId - deposited token
    // - amount - deposited value
    // - fee - fee
    // - franlkinAddress - address of Franklin account whtere deposit will be sent
    event OnchainDeposit(
        address indexed owner,
        uint16 tokenId,
        uint128 amount,
        uint256 fee,
        bytes franklinAddress
    );

    // Event emitted when blocks are reverted
    // Structure:
    // - totalBlocksVerified - number of verified blocks
    // - totalBlocksCommitted - number of committed blocks
    event BlocksReverted(
        uint32 indexed totalBlocksVerified,
        uint32 indexed totalBlocksCommitted
    );

    // Exodus mode entered event
    event ExodusMode();

    // Root-chain balances (per owner and token id) to withdraw
    mapping(address => mapping(uint16 => uint128)) public balancesToWithdraw;

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

    // Flag indicating that a user has exited certain token balance (per owner and tokenId)
    mapping(address => mapping(uint16 => bool)) public exited;

    // Flag indicating that exodus (mass exit) mode is triggered
    // Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

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
        address _governanceAddress,
        address _verifierAddress,
        address _priorityQueueAddress,
        address _genesisAccAddress,
        bytes32 _genesisRoot
    ) public {
        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);
        priorityQueue = PriorityQueue(_priorityQueueAddress);

        blocks[0].stateRoot = _genesisRoot;
    }

    // Collects fees from provided requests number for the block validator, store it on her
    // balance to withdraw in Ether and delete this requests
    // Params:
    // - _number - the number of requests
    // - _validator - address to pay fee
    function collectValidatorsFeeAndDeleteRequests(uint64 _number, address _validator) internal {
        uint256 totalFee = priorityQueue.collectValidatorsFeeAndDeleteRequests(_number);
        balancesToWithdraw[_validator][0] += uint128(totalFee);
    }

    // Accrues users balances from deposit priority requests
    // WARNING: Only for Exodus mode
    function cancelOutstandingDepositsForExodusMode() internal {
        bytes memory depositsPubData = priorityQueue.getOutstandingDeposits();
        uint64 i = 0;
        while (i < depositsPubData.length) {
            bytes memory deposit = Bytes.slice(depositsPubData, i, ETH_ADDR_BYTES+TOKEN_BYTES+AMOUNT_BYTES);
            bytes memory owner = new bytes(ETH_ADDR_BYTES);
            for (uint8 j = 0; j < ETH_ADDR_BYTES; ++j) {
                owner[j] = deposit[j];
            }
            bytes memory token = new bytes(TOKEN_BYTES);
            for (uint8 j = 0; j < TOKEN_BYTES; j++) {
                token[j] = deposit[ETH_ADDR_BYTES + j];
            }
            bytes memory amount = new bytes(AMOUNT_BYTES);
            for (uint8 j = 0; j < AMOUNT_BYTES; ++j) {
                amount[j] = deposit[ETH_ADDR_BYTES + TOKEN_BYTES + j];
            }
            balancesToWithdraw[Bytes.bytesToAddress(owner)][Bytes.bytesToUInt16(token)] += Bytes.bytesToUInt128(amount);
            i += ETH_ADDR_BYTES+TOKEN_BYTES+AMOUNT_BYTES+PUBKEY_HASH_LEN;
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

    // Deposit ETH
    // Params:
    // - _franklinAddr - the receiver Franklin address
    function depositETH(uint128 _amount, bytes calldata _franklinAddr) external payable {
        // Fee is:
        //   fee coeff * base tx gas cost * gas price
        uint256 fee = FEE_COEFF * BASE_DEPOSIT_ETH_GAS * tx.gasprice;

        requireActive();

        require(
            msg.value >= fee + _amount,
            "fdh11"
        ); // fdh11 - Not enough ETH provided
        
        require(
            _amount <= MAX_VALUE,
            "fdh12"
        ); // fdh12 - deposit amount value is heigher than Franklin is able to process

        if (msg.value != fee + _amount) {
            msg.sender.transfer(msg.value-(fee + _amount));
        }

        registerDeposit(0, _amount, fee, _franklinAddr);
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
        //   fee coeff * base tx gas cost * gas price
        uint256 fee = FEE_COEFF * BASE_DEPOSIT_ERC_GAS * tx.gasprice;

        requireActive();

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(_token);

        require(
            msg.value >= fee,
            "fd011"
        ); // fd011 - Not enough ETH provided to pay the fee

        require(
            IERC20(_token).transferFrom(msg.sender, address(this), _amount),
            "fd012"
        ); // fd012 - token transfer failed deposit

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
    // - _accountId - numerical id of the account
    // - _pubKey - packed public key of the user account
    // - _token - token address, 0 address for ether
    // - _signature - user signature
    // - _nonce - request nonce
    function fullExit (
        uint24 _accountId,
        bytes calldata _pubKey,
        address _token,
        bytes calldata _signature,
        uint32 _nonce
    ) external payable {
        // Fee is:
        //   fee coeff * base tx gas cost * gas price
        uint256 fee = FEE_COEFF * BASE_FULL_EXIT_GAS * tx.gasprice;
        
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

        require(
            _pubKey.length == PUBKEY_LEN,
            "fft13"
        ); // fft13 - wrong pubkey length

        // Priority Queue request
        bytes memory pubData = Bytes.toBytesFromUInt24(_accountId); // franklin id
        pubData = Bytes.concat(pubData, _pubKey); // account id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromAddress(msg.sender)); // eth address
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt16(tokenId)); // token id
        pubData = Bytes.concat(pubData, Bytes.toBytesFromUInt32(_nonce)); // nonce
        pubData = Bytes.concat(pubData, _signature); // signature

        priorityQueue.addPriorityRequest(uint8(OpType.FullExit), fee, pubData);
        
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

        priorityQueue.addPriorityRequest(uint8(OpType.Deposit), _fee, pubData);

        emit OnchainDeposit(
            msg.sender,
            _token,
            _amount,
            _fee,
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
            governance.isValidator(msg.sender),
            "fck12"
        ); // fck12 - not a validator in commit
        if(!triggerRevertIfBlockCommitmentExpired() && !triggerExodusIfNeeded()) {
            require(
                totalBlocksCommitted - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS,
                "fck13"
            ); // fck13 - too many committed
            
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

            priorityQueue.increaseCommittedRequestsNumber(priorityNumber);

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
            "fcs11"
        ); // fcs11 - pubdata.len % 8 != 0

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
            currentOnchainOp += ops;
        }
        require(
            currentPointer == _publicData.length,
            "fcs12"
        ); // fcs12 - last chunk exceeds pubdata
    }

    // Returns operation processed length, and indicators if this operation is
    // an onchain operation and it is a priority operation (1 if true)
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
        uint256 opDataPointer = _currentPointer + 1; // operation type byte

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
            bytes memory pubData = Bytes.slice(_publicData, opDataPointer, ACC_NUM_BYTES + PUBKEY_LEN + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_LEN + AMOUNT_BYTES);
            require(
                pubData.length == ACC_NUM_BYTES + PUBKEY_LEN + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_LEN + AMOUNT_BYTES,
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
        priorityQueue.validateNumberOfRequests(_number);
        
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;
        
        uint64 counter = 0;
        for (uint64 current = start; current < end; ++current) {
            if (onchainOps[current].opType == OpType.FullExit || onchainOps[current].opType == OpType.Deposit) {
                OnchainOperation memory op = onchainOps[current];
                require(
                    priorityQueue.isPriorityOpValid(uint8(op.opType), op.pubData, counter),
                    "fvs11"
                ); // fvs11 - priority operation is not valid
                counter++;
            }
        }
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

        require(
            verifier.verifyBlockProof(_proof, blocks[_blockNumber].commitment),
            "fvk13"
        ); // fvk13 - verification failed

        consummateOnchainOps(_blockNumber);

        collectValidatorsFeeAndDeleteRequests(
            blocks[_blockNumber].priorityOperations,
            blocks[_blockNumber].validator
        );

        totalBlocksVerified += 1;

        emit BlockVerified(_blockNumber);
    }

    // TODO: Temp. solution.
    // When withdraw is verified we move funds to the user immediately, so that withdraw can be completed with one op.
    function payoutWithdrawNow(address _to, uint16 _tokenId, uint128 _amount) internal {
        if (_tokenId == 0) {
            address payable to = address(uint160(_to));
            if (!to.send(_amount)) {
                balancesToWithdraw[_to][_tokenId] += _amount;
            }
        } else if (governance.isValidTokenId(_tokenId)) {
            address tokenAddr = governance.tokenAddresses(_tokenId);
            if(!IERC20(tokenAddr).transfer(_to, _amount)) {
                balancesToWithdraw[_to][_tokenId] += _amount;
            }
        }
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
                payoutWithdrawNow(Bytes.bytesToAddress(ethAddress), tokenId, amount);
            }
            if (op.opType == OpType.FullExit) {
                // full exit was successful, accrue balance
                bytes memory tokenBytes = new bytes(TOKEN_BYTES);
                for (uint8 i = 0; i < TOKEN_BYTES; ++i) {
                    tokenBytes[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_LEN + ETH_ADDR_BYTES + i];
                }
                uint16 tokenId = Bytes.bytesToUInt16(tokenBytes);

                bytes memory amountBytes = new bytes(AMOUNT_BYTES);
                for (uint256 i = 0; i < AMOUNT_BYTES; ++i) {
                    amountBytes[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_LEN + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_LEN + i];
                }
                uint128 amount = Bytes.bytesToUInt128(amountBytes);

                bytes memory ethAddress = new bytes(ETH_ADDR_BYTES);
                for (uint256 i = 0; i < ETH_ADDR_BYTES; ++i) {
                    ethAddress[i] = op.pubData[ACC_NUM_BYTES + PUBKEY_LEN + i];
                }
                payoutWithdrawNow(Bytes.bytesToAddress(ethAddress), tokenId, amount);
            }
            delete onchainOps[current];
        }
    }

    // Checks that commitment is expired and revert blocks
    function triggerRevertIfBlockCommitmentExpired() internal returns (bool) {
        if (
            totalBlocksCommitted > totalBlocksVerified &&
            blocks[totalBlocksVerified + 1].committedAtBlock > 0 &&
            block.number > blocks[totalBlocksVerified + 1].committedAtBlock + EXPECT_VERIFICATION_IN
        ) {
            revertBlocks();
            return true;
        }
        return false;
    }

    // Reverts unverified blocks
    function revertBlocks() internal {
        for (uint32 i = totalBlocksVerified + 1; i <= totalBlocksCommitted; i++) {
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
        priorityQueue.decreaseCommittedRequestsNumber(_reverted.priorityOperations);
    }

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
        if (priorityQueue.triggerExodusIfNeeded()) {
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
        address _owner,
        uint128 _amount,
        uint256[8] calldata _proof
    ) external {
        require(
            exodusMode,
            "fet11"
        ); // fet11 - must be in exodus mode

        require(
            exited[_owner][_tokenId] == false,
            "fet12"
        ); // fet12 - already exited

        require(
            verifier.verifyExitProof(_tokenId, _owner, _amount, _proof),
            "fet13"
        ); // fet13 - verification failed

        balancesToWithdraw[_owner][_tokenId] += _amount;
        exited[_owner][_tokenId] == false;
    }
}
