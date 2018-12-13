pragma solidity ^0.4.24;

import "./Verifier.sol";
import "./VerificationKeys.sol";

// Single operator mode

contract PlasmaStub is VerificationKeys {

    uint32 constant DEADLINE = 3600; // seconds, to define

    event BlockCommitted(uint32 indexed blockNumber);
    event BlockVerified(uint32 indexed blockNumber);

    enum Circuit {
        DEPOSIT,
        UPDATE,
        WITHDRAWAL
    }

    uint8 constant CIRCUIT_TYPE_NULL = 0;
    uint8 constant CIRCUIT_TYPE_DEPOSIT = 1;
    uint8 constant CIRCUIT_TYPE_TRANSFER = 2;
    uint8 constant CIRCUIT_TYPE_EXIT = 3;

    struct Block {
        uint8 circuit;
        uint64  deadline;
        uint128 totalFees;
        bytes32 newRoot;
        bytes32 publicDataCommitment;
        address prover;
    }

    // Key is block number
    mapping (uint32 => Block) public blocks;
    // Only some addresses can send proofs
    mapping (address => bool) public operators;
    // Some dividends distribution
    mapping (address => uint256) public balancs;

    // Public information for users
    bool public stopped;
    uint32 public totalCommitted;
    uint32 public totalVerified;
    bytes32 public lastVerifiedRoot;
    uint64 public constant maxAllowedOperatorsDelay = 1 days;
    uint64 public lastProcessesDepositTimestamp;
    uint64 public lastProcessesExitTimestamp;

    modifier operator_only() {
        require(!stopped, "contract should not be globally stopped");
        require(operators[msg.sender] == true, "sender should be one of the operators");
        _;
    }

    // constructor() public {
    //     lastVerifiedRoot = EMPTY_TREE_ROOT;
    //     operators[msg.sender] = true;
    // }

    // on commitment to some block we just commit to SOME public data, that will be parsed 
    // ONLY when proof is presented
    
    function commitDepositBlock(uint32 blockNumber, bytes memory txDataPacked, bytes32 newRoot) public operator_only {
        require(blockNumber == totalCommitted + 1, "may only commit next block");

        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForDeposit(blockNumber, txDataPacked);

        blocks[blockNumber] = Block(CIRCUIT_TYPE_DEPOSIT, uint64(block.timestamp + DEADLINE), totalFees, newRoot, publicDataCommitment, msg.sender);
        emit BlockCommitted(blockNumber);
        totalCommitted++;
    }

    function commitTransferBlock(uint32 blockNumber, uint128 totalFees, bytes memory txDataPacked, bytes32 newRoot) public operator_only {
        require(blockNumber == totalCommitted + 1, "may only commit next block");

        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForTransfer(blockNumber, totalFees, txDataPacked);

        blocks[blockNumber] = Block(CIRCUIT_TYPE_TRANSFER, uint64(block.timestamp + DEADLINE), totalFees, newRoot, publicDataCommitment, msg.sender);
        emit BlockCommitted(blockNumber);
        totalCommitted++;
    }

    function commitExitBlock(uint32 blockNumber, uint128 totalFees, bytes memory txDataPacked, bytes32 newRoot) operator_only public operator_only {
        require(blockNumber == totalCommitted + 1, "may only commit next block");

        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForExit(blockNumber, txDataPacked);

        blocks[blockNumber] = Block(CIRCUIT_TYPE_EXIT, uint64(block.timestamp + DEADLINE), totalFees, newRoot, publicDataCommitment, msg.sender);
        emit BlockCommitted(blockNumber);
        totalCommitted++;
    }

    function verifyTransferBlock(uint32 blockNumber, uint256[8] memory proof) public operator_only {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        Block memory committed = blocks[blockNumber];
        require(committed.circuit == CIRCUIT_TYPE_TRANSFER, "trying to prove the invalid circuit for this block number");
        bool verification_success = verifyTransferProof(proof, lastVerifiedRoot, committed.newRoot, committed.publicDataCommitment);
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedRoot = committed.newRoot;

        // TODO: how to deal with deadline? Penalties?
        balance[committed.prover] += committed.totalFees;
    }

    function verifyDepositBlock(uint32 blockNumber, uint256[8] memory proof, bytes memory txDataPacked) public operator_only {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        Block memory committed = blocks[blockNumber];
        require(committed.circuit == CIRCUIT_TYPE_DEPOSIT, "trying to prove the invalid circuit for this block number");
        bytes32 publicDataCommitment = createPublicDataCommitmentForDeposit(blockNumber, txDataPacked);
        require(committed.publicDataCommitment == publicDataCommitment, "block data is different from committed one");
        bool verification_success = verifyDepositProof(proof, lastVerifiedRoot, committed.newRoot, committed.publicDataCommitment);
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedRoot = committed.newRoot;

        // process the block information

        // TODO: how to deal with deadline? Penalties?
        balance[committed.prover] += committed.totalFees;
    }

    function verifyExitBlock(uint32 blockNumber, uint256[8] memory proof, bytes memory txDataPacked) public operator_only {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        Block memory committed = blocks[blockNumber];
        require(committed.circuit == CIRCUIT_TYPE_EXIT, "trying to prove the invalid circuit for this block number");
        bytes32 publicDataCommitment = createPublicDataCommitmentForExit(blockNumber, txDataPacked);
        require(committed.publicDataCommitment == publicDataCommitment, "block data is different from committed one");
        bool verification_success = verifyExitProof(proof, lastVerifiedRoot, committed.newRoot, committed.publicDataCommitment);
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedRoot = committed.newRoot;

        // process the block information

        // TODO: how to deal with deadline? Penalties?
        balance[committed.prover] += committed.totalFees;
    }

    // block processing functions that modity the Ethereum state



    // pure functions to calculate commitment formats
    function createPublicDataCommitmentForDeposit(uint32 blockNumber, uint128 totalFees, bytes memory txDataPacked)
    public 
    pure
    returns (bytes32 h) {

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        
        return finalHash;
    }

    // pure functions to calculate commitment formats
    function createPublicDataCommitmentForTransfer(uint32 blockNumber, uint128 totalFees, bytes memory txDataPacked)
    public 
    pure
    returns (bytes32 h) {

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber), uint256(totalFees)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        
        return finalHash;
    }

    // pure functions to calculate commitment formats
    function createPublicDataCommitmentForExit(uint32 blockNumber, bytes memory txDataPacked)
    public 
    pure
    returns (bytes32 h) {

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        
        return finalHash;
    }

    // stubs
    // data processing
    function processDepositBlockData(bytes memory) internal view;
    function processExitBlockData(bytes memory) internal view;
    // verification
    function verifyDepositProof(uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid);
    function verifyTransferProof(uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid);
    function verifyExitProof(uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid);
}


contract Plasma is PlasmaStub, Verifier {
    // Implementation

    function verifyDepositProof(uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVkDepositCircuit();
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

    function verifyTransferProof(uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVkTransferCircuit();
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

    function verifyExitProof(uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVkExitCircuit();
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

}

contract PlasmaUserInterface is Plasma {

    struct DepositRequest {
        uint128 amount;
        uint64 timestamp;
        uint8 processed;
    }

    struct ExitRequest {
        uint64 timestamp;
        uint8 processed;
    }

    struct Account {
        address owner;
        uint256 publicKey;
        deposit DepositRequest;
        exit ExitRequest;
    }

    event DepositRequestEvent(uint24 indexed accountID, uin256 indexed publicKey, uint128 amount);
    event ExitRequestEvent(uint24 indexed account);
    // accounts 0 and 1 are taken by the operator
    uint256 constant operatorsAccounts = 16;
    uint256 public nextAccountToRegisted = operatorsAccounts;
    mapping (address => uint24) public ethereumAddressToAccountID;
    mapping (uint24 => Account) public accounts;

    // create technological accounts for an operator. 
    constructor(uint256[operatorsAccounts] memory defaultPublicKeys) public {
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        for (uint256 i = 0; i < operatorsAccounts; i++) {
            Account storage acc = accounts[i];
            acc.owner = msg.sender;
            acc.publicKey = defaultPublicKeys[i];
        }
    }

    // transaction data is trivial: 3 bytes of in-plasma address and 16 bytes of amount
    function processDepositBlockData(bytes memory txData) 
    internal 
    view
    {
        // use uint256 to read data into
        uint256 chunk;
        uint256 pointer = 0;
        uint256 numberOfTransactions = txData.length;
        uint24[] memory accountIDs = new uint24[](numberOfTransactions);
        uint24[] memory amounts = new uint24[](numberOfTransactions);
        for (uint256 i = 0; i < numberOfTransactions; i++) {
            // we don't care about reading pass the array because later we clear those bits
            pointer = 19 * i;
            assembly {
                chunk := mload(add(add(txData, 0x20), pointer))
            }
            accountIDs[i] = chunk >> 232;
            amounts[i] = chunk << 24 >> 128;
        }
        // transaction data is parsed, so read the state and modify
        uint64 lastProcessedExitTimestamp;
        for (uint256 i = 0; i < numberOfTransactions; i++) {
            Account storage thisAccount = accounst[accountIDs[i]];
            DepositRequest storage request = thisAccount.deposit;
            require(request.processed == 0, "trying to process an already processes request");
            request.processed = 1;
            lastProcessedExitTimestamp = request.timestamp;
            require(request.amount == amounts[i], "deposit request amount is not equal to the proved amount");
        }

        lastProcessesDepositTimestamp = lastProcessedExitTimestamp;
    }

    // transaction data is trivial: 3 bytes of in-plasma address and 32 bytes of the compressed public key
    function processExitBlockData(bytes memory) 
    internal 
    view
    {

    }


    // user exposed functions: deposits requests, exit requests, take back the deposit if user is censored

    function deposit(uint256[2] publicKey)
    public
    payable
    {
        uint24 existingID = ethereumAddressToAccountID[msg.sender];
        require(msg.value % 1000000000000 == 0, "amount has higher precision than possible");
        require(msg.value / 1000000000000 < uint256(1) << 128, "deposit amount is too high");
        uint128 depositAmount = uint128(msg.value / 1000000000000);
        if (existingID == 0 && !operators[msg.sender]) {
            ethereumAddressToAccountID[msg.sender] = nextAccountToRegister;
            Account storage freshAccount = accounts[nextAccountToRegister];
            freshAccount.owner = msg.sender;
            DepositRequest memory firstRequest = new DepositRequest ({
                amount: depositAmount,
                timestamp: block.timestamp,
                processed: 0
            });
            freshAccount.deposit = firstRequest;
            uint256 packedKey = packAndValidatePublicKey(publicKey);
            require(packedKey != 0, "invalid pyblic key to register");
            emit DepositRequestEvent(nextAccountToRegister, packedKey, depositAmount);
            nextAccountToRegister += 1;
        } else {
            Account storage existingAccount = accounts[existingID];
            DepositRequest storage request = existingAccount.deposit;
            require(request.processed == 0, "there is already a pending request for a deposit for this account");
            request.timestamp = block.timestamp;
            request.amount = depositAmount;
            emit DepositRequestEvent(existingID, existingAccount.publicKey, depositAmount);
        }
    }

    function packAndValidatePublicKey(uint256[2] memory publicKey)
    public
    pure
    returns(uint256 packed) {
        // group check + packing
        return publicKey[1];
    }

}
