pragma solidity ^0.5.1;

import "openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

// Warning! Verifier does not work.
//import "./Verifier.sol";
import "./common/DummyVerifier.sol";
import "./common/VerificationKeys.sol";

contract Franklin is DummyVerifier, VerificationKey {

    uint constant BLOCK_SIZE = 2000;                // chunks per block; each chunk has 8 bytes of public data
    uint constant MAX_VALUE = 2**112-1;             // must fit into uint112
    uint constant LOCK_DEPOSITS_FOR = 8*60;         // ETH blocks
    uint constant EXPECT_VERIFICATION_IN = 8*60;    // ETH blocks
    uint constant MAX_UNVERIFIED_BLOCKS = 4*60;     // To make sure that all reverted blocks can be copied under block gas limit!

    event BlockCommitted(uint32 indexed blockNumber);
    event BlockVerified(uint32 indexed blockNumber);
    event BlocksReverted(uint32 indexed totalBlocksVerified, uint32 indexed totalBlocksCommitted);

    event OnchainDeposit(address indexed owner, uint32 tokenId, uint112 amount, uint32 lockedUntilBlock);
    event OnchainWithdrawal(address indexed owner, uint32 tokenId, uint112 amount);
    event OnchainBalanceChanged(address indexed owner, uint32 tokenId, uint112 amount, uint32 lockedUntilBlock);

    event TokenAdded(address token, uint32 tokenId);

    event AccountRegistered(address indexed owner, uint32 id);

    // ==== STORAGE ====

    // Governance

    // Address which will excercise governance over the network
    // i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernor;

    // Total number of ERC20 tokens registered in the network
    // (excluding ETH, which is hardcoded as tokenId = 0)
    uint32 public totalTokens;

    // List of registered tokens by tokenId
    mapping (uint32 => address) public tokenAddresses;

    // List of registered tokens by address
    mapping (address => uint32) public tokenIds;

    // List of permitted validators
    mapping (address => bool) public validators;


    // Root-chain balances

    // Root-chain balance: users can send funds from and to Franklin
    // from the root-chain balances only (see docs)
    struct Balance {
        uint112 balance;

        // Balance can be locked in order to let validators deposit some of it into Franklin
        // Locked balances becomes free at ETH blockNumber = lockedUntilBlock
        // To re-lock, users can make a deposit with zero amount
        uint32  lockedUntilBlock;
    }

    // List of root-chain balances (per owner and tokenId)
    mapping (address => mapping (uint32 => Balance)) public balances;

    uint32 public totalAccounts;
    mapping (address => uint32) public accountIdByAddress;


    // Blocks

    // Total number of verified blocks
    // i.e. blocks[totalBlocksVerified] points at the latest verified block (block 0 is genesis)
    uint32 public totalBlocksVerified;

    // Total number of committed blocks
    // i.e. blocks[totalBlocksCommitted] points at the latest committed block
    uint32 public totalBlocksCommitted;

    // Block data (once per block)
    struct Block {
        // Hash of committment the block circuit
        bytes32 commitment;

        // New root hash
        bytes32 stateRoot;

        // ETH block number at which this block was committed
        uint32  committedAtBlock;

        // ETH block number at which this block was verified
        uint32  verifiedAtBlock;

        // Validator (aka block producer)
        address validator;

        // Index of the first operation to process for this block
        uint64  operationStartId;

        // Total number of operations to process for this block
        uint64  totalOperations;
    }

    // List of blocks by Franklin blockId
    mapping (uint32 => Block) public blocks;


    // Onchain operations -- processed inside blocks (see docs)

    // Type of block processing operation holder
    enum OnchainOpType {
        Deposit,
        Withdrawal
    }

    // OnchainOp keeps a balance for processing the committed data in blocks, see docs
    struct OnchainOp {
        OnchainOpType   opType;
        uint32          tokenId;
        address         owner;
        uint112         amount;
    }

    // Total number of registered OnchainOps
    uint64 totalOnchainOps;

    // List of OnchainOps by index
    mapping (uint64 => OnchainOp) onchainOps;


    // Reverting expired blocks

    // Total number of registered blocks to revert (see docs)
    uint32 totalBlocksToRevert;

    // List of blocks by revertBlockId (see docs)
    mapping (uint32 => Block) public blocksToRevert;


    // Exit queue & exodus mode

    // Address of the account which is allowed to trigger exodus mode
    // (mass exits in the case that censorship resistance has failed)
    address public exitQueue;

    // Flag indicating that exodus (mass exit) mode is triggered
    // Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

    // Flag indicating that a user has exited certain token balance (per owner and tokenId)
    mapping (address => mapping (uint32 => bool)) public exited;


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


    // ==== IMPLEMENTATION ====

    // Constructor

    constructor(bytes32 _genesisRoot, address _exitQueue, address _networkGovernor) public {
        blocks[0].stateRoot = _genesisRoot;
        exitQueue = _exitQueue;
        networkGovernor = _networkGovernor;

        // TODO: remove once proper governance is implemented
        validators[_networkGovernor] = true;
    }


    // Governance

    function changeGovernor(address _newGovernor) external {
        requireGovernor();
        networkGovernor = _newGovernor;
    }

    function addToken(address _token) external {
        requireGovernor();
        require(tokenIds[_token] == 0, "token exists");
        tokenAddresses[totalTokens + 1] = _token; // Adding one because tokenId = 0 is reserved for ETH
        tokenIds[_token] = totalTokens + 1;
        totalTokens++;
        emit TokenAdded(_token, totalTokens);
    }

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


    // Root-chain balances

    // Deposit ETH (simply by sending it to the contract)
    function() external payable {
        require(msg.value <= MAX_VALUE, "sorry Joe");
        registerDeposit(0, uint112(msg.value));
    }

    function withdrawETH(uint112 _amount) external {
        registerWithdrawal(0, _amount);
        msg.sender.transfer(_amount);
    }

    function depositERC20(address _token, uint112 _amount) external {
        require(IERC20(_token).transferFrom(msg.sender, address(this), _amount), "transfer failed");
        registerDeposit(tokenIds[_token], _amount);
    }

    function withdrawERC20(address _token, uint112 _amount) external {
        registerWithdrawal(tokenIds[_token], _amount);
        require(IERC20(_token).transfer(msg.sender, _amount), "transfer failed");
    }

    function registerDeposit(uint32 _tokenId, uint112 _amount) internal {
        requireActive();
        requireValidToken(_tokenId);
        require(uint256(_amount) + balances[msg.sender][_tokenId].balance < MAX_VALUE, "overflow");

        // register account if not registered
        if (accountIdByAddress[msg.sender] == 0) {
            accountIdByAddress[msg.sender] = totalAccounts + 1;
            totalAccounts++;
        }

        balances[msg.sender][_tokenId].balance += _amount;
        uint32 lockedUntilBlock = uint32(block.number + LOCK_DEPOSITS_FOR);
        balances[msg.sender][_tokenId].lockedUntilBlock = lockedUntilBlock;
        emit OnchainBalanceChanged(msg.sender, _tokenId, balances[msg.sender][_tokenId].balance, lockedUntilBlock);
        emit OnchainDeposit(msg.sender, _tokenId, _amount, lockedUntilBlock);
    }

    function registerWithdrawal(uint32 _tokenId, uint112 _amount) internal {
        requireActive();
        requireValidToken(_tokenId);
        require(block.number >= balances[msg.sender][_tokenId].lockedUntilBlock, "balance locked");
        require(balances[msg.sender][_tokenId].balance >= _amount, "insufficient balance");
        balances[msg.sender][_tokenId].balance -= _amount;
        emit OnchainBalanceChanged(msg.sender, _tokenId, balances[msg.sender][_tokenId].balance, balances[msg.sender][_tokenId].lockedUntilBlock);
        emit OnchainWithdrawal(msg.sender, _tokenId, _amount);
    }


    // Block committment

    function commitBlock(uint32 _blockNumber, bytes32 _newRoot, bytes calldata _publicData) external {
        requireActive();
        require(validators[msg.sender], "only by validator");
        require(_blockNumber == totalBlocksCommitted + 1, "only commit next block");
        require(blockCommitmentExpired() == false, "committment expired");
        require(totalBlocksCommitted - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS, "too many committed");

        // TODO: check exit queue: it will require certains records to appear on `_publicData`

        // TODO: make efficient padding here

        (uint64 startId, uint64 totalProcessed) = commitOnchainOps(_publicData);

        bytes32 commitment = createBlockCommitment(
            _blockNumber,
            msg.sender,
            blocks[_blockNumber - 1].stateRoot,
            _newRoot,
            _publicData);

        blocks[_blockNumber] = Block(
            commitment,
            _newRoot,
            _blockNumber,   // committed at
            0,              // verified at
            msg.sender,     // validator

            // onchain-ops
            startId,
            totalProcessed
        );

        totalBlocksCommitted += 1;
        emit BlockCommitted(_blockNumber);
    }

    function createBlockCommitment(uint32 _blockNumber, address _validator, bytes32 _oldRoot, bytes32 _newRoot, bytes memory _publicData)
        internal pure returns (bytes32)
    {
        bytes32 hash = sha256(abi.encodePacked(uint256(_blockNumber), uint256(_validator)));
        hash = sha256(abi.encodePacked(hash, _oldRoot));
        hash = sha256(abi.encodePacked(hash, _newRoot));
        // public data is committed with padding (TODO: check assembly and optimize to avoid copying data)
        hash = sha256(abi.encodePacked(hash, _publicData, new bytes(BLOCK_SIZE * 8 - _publicData.length)));
        return hash;
    }

    function commitOnchainOps(bytes memory _publicData)
        internal returns (uint64 onchainOpsStartId, uint64 processedOnchainOps)
    {
        require(_publicData.length % 8 == 0, "pubdata.len % 8 != 0");

        onchainOpsStartId = totalOnchainOps;
        uint64 currentOnchainOp = totalOnchainOps;

        // NOTE: the stuff below is the most expensive and most frequently used part of the entire contract.
        // It is highly unoptimized and can be improved by an order of magnitude by getting rid of the subroutine,
        // using assembly, replacing ifs with mem lookups and other tricks
        // TODO: optimize
        uint currentPointer = 0;
        while(currentPointer < _publicData.length) {
            bytes1 opType = _publicData[currentPointer];
            (uint len, uint64 ops) = processOp(opType, currentPointer, _publicData, currentOnchainOp);
            currentPointer += len;
            processedOnchainOps += ops;
        }
        require(currentPointer == _publicData.length, "last chunk exceeds pubdata");
        return (0, 0);
    }

    function processOp(bytes1 opType, uint currentPointer, bytes memory _publicData, uint64 currentOnchainOp)
        internal returns (uint processedLen, uint64 processedOnchainOps)
    {

        if (opType == 0x00) return (1*8, 0); // noop
        if (opType == 0x01) return (4*8, 0); // transfer_to_new
        if (opType == 0x07) return (2*8, 0); // transfer
        if (opType == 0x05) return (1*8, 0); // close_account

        // deposit
        if (opType == 0x01) {
            // pubdata: to_account: 3, token: 2, amount: 2, fee: 1, new_pubkey_hash: 21

            // TODO: - this should work: get address by id
            address account = address(uint(uint8(_publicData[currentPointer + 1])) + uint(uint8(_publicData[currentPointer + 2])) << 8 + uint(uint8(_publicData[currentPointer + 3])) << 16);
            
            uint16 tokenId = uint16(uint(uint8(_publicData[currentPointer + 4])) + uint(uint8(_publicData[currentPointer + 5])) << 8);
            uint24 amountPacked = uint24(uint8(_publicData[currentPointer + 6])) + uint24(uint8(_publicData[currentPointer + 7])) << 8;
            uint112 amount = unpack(amountPacked, tokenId);

            requireValidToken(tokenId);
            require(block.number < balances[account][tokenId].lockedUntilBlock, "balance must be locked");
            require(balances[account][tokenId].balance >= amount, "balance insuffcient");

            balances[account][tokenId].balance -= amount;
            emit OnchainBalanceChanged(account, tokenId, balances[account][tokenId].balance, balances[account][tokenId].lockedUntilBlock);
            onchainOps[currentOnchainOp] = OnchainOp(
                OnchainOpType.Deposit,
                tokenId,
                account,
                amount
            );
            return (4*8, 1);
        }

        // partial_exit
        if (opType == 0x04) {
            // pubdata: account: 3, token: 2, amount: 2, fee: 1

            // TODO: - this should work: get address by id
            address account = address(uint(uint8(_publicData[currentPointer + 1])) + uint(uint8(_publicData[currentPointer + 2])) << 8 + uint(uint8(_publicData[currentPointer + 3])) << 16);
            
            uint16 tokenId = uint16(uint(uint8(_publicData[currentPointer + 4])) + uint(uint8(_publicData[currentPointer + 5])) << 8);
            uint24 amountPacked = uint24(uint8(_publicData[currentPointer + 6])) + uint24(uint8(_publicData[currentPointer + 7])) << 8;
            uint112 amount = unpack(amountPacked, tokenId);

            requireValidToken(tokenId);

            onchainOps[currentOnchainOp] = OnchainOp(
                OnchainOpType.Withdrawal,
                tokenId,
                account,
                amount
            );
            return (2*8, 1);
        }

        require(false, "unsupported op");
    }


    // Block verification

    function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
        requireActive();
        require(validators[msg.sender], "only by validator");
        require(_blockNumber == totalBlocksVerified + 1, "only verify next block");

        require(verifyBlockProof(proof, blocks[_blockNumber].commitment), "verification failed");

        totalBlocksVerified += 1;
        consummateOnchainOps(_blockNumber);
        emit BlockVerified(_blockNumber);
    }

    function verifyBlockProof(uint256[8] memory proof, bytes32 commitment)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(commitment) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 current = blocks[_blockNumber].operationStartId;
        uint64 end = current + blocks[_blockNumber].totalOperations;
        while (current < end) {
            OnchainOp memory op = onchainOps[current];
            if (op.opType == OnchainOpType.Withdrawal) {
                // withdrawal was successful, accrue balance
                balances[op.owner][op.tokenId].balance += op.amount;
                emit OnchainBalanceChanged(op.owner, op.tokenId, balances[op.owner][op.tokenId].balance, balances[op.owner][op.tokenId].lockedUntilBlock);
            }
            delete onchainOps[current];
        }
    }


    // Reverting committed blocks

    function revertExpiredBlocks() external {
        require(blockCommitmentExpired(), "not expirated");
        emit BlocksReverted(totalBlocksVerified, totalBlocksCommitted);
        uint32 total = totalBlocksCommitted - totalBlocksVerified;
        for(uint32 i = 0; i < total; i++) {
            blocksToRevert[totalBlocksToRevert + i] = blocks[totalBlocksVerified + i + 1];
            delete blocks[totalBlocksVerified + i + 1];
        }
        totalBlocksToRevert += total;
    }

    function revertBlock(uint32 _revertedBlockId) external {
        Block memory reverted = blocksToRevert[_revertedBlockId];
        require(reverted.committedAtBlock > 0, "block not found");

        uint64 current = reverted.operationStartId;
        uint64 end = current + reverted.totalOperations;
        while (current < end) {
            OnchainOp memory op = onchainOps[current];
            if (op.opType == OnchainOpType.Deposit) {
                // deposit failed, return funds
                balances[op.owner][op.tokenId].balance += op.amount;
                emit OnchainBalanceChanged(op.owner, op.tokenId, balances[op.owner][op.tokenId].balance, balances[op.owner][op.tokenId].lockedUntilBlock);
            }
            delete onchainOps[current];
        }
        delete blocksToRevert[_revertedBlockId];
    }


    // Exodus mode

    function triggerExodus() external {
        require(msg.sender == exitQueue, "only by exit queue");
        exodusMode = true;
    }

    function exit(uint32 _tokenId, address[] calldata _owners, uint112[] calldata _amounts, uint256[8] calldata /*_proof*/) external {
        require(exodusMode, "must be in exodus mode");
        require(_owners.length == _amounts.length, "|owners| != |amounts|");

        for(uint256 i = 0; i < _owners.length; i++) {
            require(exited[_owners[i]][_tokenId] == false, "already exited");
        }

        // TODO: verify the proof that all users have the specified amounts of this token in the latest state

        for(uint256 i = 0; i < _owners.length; i++) {
            balances [_owners[i]][_tokenId].balance += _amounts[i];
            emit OnchainBalanceChanged(_owners[i], _tokenId, balances[_owners[i]][_tokenId].balance, balances[_owners[i]][_tokenId].lockedUntilBlock);
            exited   [_owners[i]][_tokenId] = true;
        }
    }


    // Internal helpers

    function requireGovernor() internal view {
        require(msg.sender == networkGovernor, "only by governor");
    }

    function requireActive() internal view {
        require(!exodusMode, "exodus mode");
    }

    function requireValidToken(uint32 _tokenId) internal view {
        require(_tokenId < totalTokens + 1, "unknown token");
    }

    function blockCommitmentExpired() internal view returns (bool) {
        return totalBlocksCommitted > totalBlocksVerified &&
            block.number > blocks[totalBlocksVerified + 1].committedAtBlock + EXPECT_VERIFICATION_IN;
    }

    function unpack(uint24 _amount, uint16 /*_tokenId*/) internal pure returns (uint112) {
        // TODO: implement depending on the format chosen
        return uint112(_amount >> 8) * uint112(10) ** (15 + (_amount ^ 0xff));
    }

}
