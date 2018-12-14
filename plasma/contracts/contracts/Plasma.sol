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
        TRANSFER,
        WITHDRAWAL
    }

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
    // Fee collection accounting
    mapping (address => uint256) public balances;

    struct Account {
        address owner;
        uint256 publicKey;
    }

    // one Ethereum address should have one account
    mapping (address => uint24) public ethereumAddressToAccountID;

    // Plasma account => general information
    mapping (uint24 => Account) public accounts;

    // Public information for users
    bool public stopped;
    uint32 public totalCommitted;
    uint32 public totalVerified;
    bytes32 public lastVerifiedRoot;
    uint64 public constant MAX_DELAY = 1 days;

    modifier active_only() {
        require(!stopped, "contract should not be globally stopped");
        _;
    }

    modifier operator_only() {
        require(operators[msg.sender] == true, "sender should be one of the operators");
        _;
    }

    // unit normalization functions
    function scale_into(uint256 value)
    internal
    pure
    returns (uint128) {
        require(value % 1000000000000 == 0, "amount has higher precision than possible");
        uint256 scaled = value / 1000000000000;
        require(scaled < uint256(1) << 128, "deposit amount is too high");
        return uint128(scaled);
    }


    function scale_from(uint128 value)
    internal
    pure
    returns (uint256) {
        return uint256(value) * 1000000000000;
    }

    function commitTransferBlock(
        uint32 blockNumber, 
        uint128 totalFees, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) 
    public 
    operator_only 
    {
        require(blockNumber == totalCommitted + 1, "may only commit next block");

        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForTransfer(blockNumber, totalFees, txDataPacked);

        blocks[blockNumber] = Block(
            uint8(Circuit.TRANSFER), 
            uint64(block.timestamp + DEADLINE), 
            totalFees, newRoot, 
            publicDataCommitment, 
            msg.sender
        );
        emit BlockCommitted(blockNumber);
        totalCommitted++;
    }

    function verifyTransferBlock(uint32 blockNumber, uint256[8] memory proof) public operator_only {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        Block memory committed = blocks[blockNumber];
        require(committed.circuit == uint8(Circuit.TRANSFER), "trying to prove the invalid circuit for this block number");
        bool verification_success = verifyProof(Circuit.TRANSFER, proof, lastVerifiedRoot, committed.newRoot, committed.publicDataCommitment);
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedRoot = committed.newRoot;

        balances[committed.prover] += committed.totalFees;
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
    // verification
    function verifyProof(Circuit, uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid);
}


contract Plasma is PlasmaStub, Verifier {
    // Implementation

    function verifyProof(Circuit circuitType, uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        if (circuitType == Circuit.DEPOSIT) {
            (vk, gammaABC) = getVkDepositCircuit();
        } else if (circuitType == Circuit.TRANSFER) {
            (vk, gammaABC) = getVkTransferCircuit();
        } else if (circuitType == Circuit.WITHDRAWAL) {
            (vk, gammaABC) = getVkExitCircuit();
        } else {
            return false;
        }
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

}

contract PlasmaDepositor is Plasma {

    uint256 constant DEPOSIT_BATCH_SIZE = 32;
    uint256 totalDepositRequests; // enumerates total number of deposit, starting from 0
    uint256 lastCommittedDepositBatch;
    uint256 lastVerifiedDepositBatch;
    uint128 currentDepositBatchFee; // deposit request fee scaled units

    // batch number => (plasma address => deposit information)
    mapping (uint256 => mapping (uint24 => DepositRequest)) public depositRequests;
    mapping (uint256 => DepositBatch) public depositBatches;

    struct DepositRequest {
        uint128 amount;
    }

    enum DepositBatchState {
        CREATED,
        COMMITTED,
        VERIFIED
    }

    struct DepositBatch {
        uint8 state;
        uint32 blockNumber;
        uint64 timestamp;
        uint128 batchFee;
    }

    event LogDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID, uint256 indexed publicKey, uint128 amount);
    event LogCancelDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID);
    // use first N accounts for technological purposes
    uint24 constant operatorsAccounts = 16;
    uint24 public nextAccountToRegister = operatorsAccounts;

    // create technological accounts for an operator. 
    constructor(uint256[operatorsAccounts] memory defaultPublicKeys) public {
        lastVerifiedRoot = EMPTY_TREE_ROOT;
        operators[msg.sender] = true;
        for (uint24 i = 0; i < operatorsAccounts; i++) {
            Account storage acc = accounts[i];
            acc.owner = msg.sender;
            acc.publicKey = defaultPublicKeys[i];
        }
    }

    function deposit(uint256[2] memory publicKey, uint128 maxFee) 
    public 
    payable {
        require(maxFee <= currentDepositBatchFee, "deposit fee is less than required");
        uint128 scaledValue = scale_into(msg.value);
        require(scaledValue > currentDepositBatchFee, "deposit amount should cover the fee");
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        if (accountID == 0) {
            // register new account
            uint256 packedKey = packAndValidatePublicKey(publicKey);
            ethereumAddressToAccountID[msg.sender] = nextAccountToRegister;
            Account storage freshAccount = accounts[nextAccountToRegister];
            freshAccount.owner = msg.sender;
            freshAccount.publicKey = packedKey;
            accountID = nextAccountToRegister;
            // bump accounts counter
            nextAccountToRegister += 1;
        }
        // read account info
        Account memory accountInformation = accounts[nextAccountToRegister];

        // work with a deposit
        uint256 currentBatch = totalDepositRequests/DEPOSIT_BATCH_SIZE;
        // write aux info about the batch
        DepositBatch storage batch = depositBatches[currentBatch];
        if (batch.timestamp == 0) {
            batch.state = uint8(DepositBatchState.CREATED);
        }
        batch.timestamp = uint64(block.timestamp);
        batch.batchFee = currentDepositBatchFee;
        scaledValue -= currentDepositBatchFee;
        // get request in this batch for this account
        DepositRequest storage request = depositRequests[currentBatch][accountID];
        
        if(request.amount == 0) {
            // this is a new request in this batch
            totalDepositRequests++;
        }
        request.amount += scaledValue;
        
        emit LogDepositRequest(currentBatch, accountID, accountInformation.publicKey, request.amount);
    }

    // allow users to cancel a deposit if the work on the next proof is not yet started
    function cancelDeposit()
    public
    {
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "trying to cancel a deposit for non-existing account");
        uint256 currentBatch = totalDepositRequests/DEPOSIT_BATCH_SIZE;
        DepositRequest storage request = depositRequests[currentBatch][accountID];
        uint128 depositAmount = request.amount;
        require(depositAmount > 0, "trying to cancel an empty deposit");
        emit LogCancelDepositRequest(currentBatch, accountID);
        delete depositRequests[currentBatch][accountID]; // refund gas
        totalDepositRequests--;
        msg.sender.transfer(scale_from(depositAmount));
    }

    function startNextDepositBatch(uint128 newBatchFee)
    public
    operator_only()
    {
        uint256 currentBatch = totalDepositRequests/DEPOSIT_BATCH_SIZE;
        uint256 inTheCurrentBatch = totalDepositRequests % DEPOSIT_BATCH_SIZE;
        if (inTheCurrentBatch != 0) {
            totalDepositRequests = (currentBatch + 1) * DEPOSIT_BATCH_SIZE;
        }
        currentDepositBatchFee = newBatchFee;
    }

    // pure function to calculate commitment formats
    function createPublicDataCommitmentForDeposit(uint32 blockNumber, bytes memory txDataPacked)
    public 
    pure
    returns (bytes32 h) {

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        
        return finalHash;
    }

    // on commitment to some block we just commit to SOME public data, that will be parsed 
    // ONLY when proof is presented
    
    function commitDepositBlock(
        uint256 batchNumber,
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs,
        uint32 blockNumber, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) 
    public 
    operator_only 
    {
        require(blockNumber == totalCommitted + 1, "may only commit next block");
        require(batchNumber == lastCommittedDepositBatch, "trying to commit batch out of order");
        
        DepositBatch storage batch = depositBatches[batchNumber];
        batch.state = uint8(DepositBatchState.COMMITTED);
        batch.blockNumber = blockNumber;
        batch.timestamp = uint64(block.timestamp);
        
        processDepositBlockData(batchNumber, accoundIDs, txDataPacked);
        
        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForDeposit(blockNumber, txDataPacked);

        blocks[blockNumber] = Block(
            uint8(Circuit.DEPOSIT), 
            uint64(block.timestamp + MAX_DELAY), 
            0, 
            newRoot, 
            publicDataCommitment, 
            msg.sender
        );
        emit BlockCommitted(blockNumber);
        totalCommitted++;
        lastCommittedDepositBatch++;
    }

    function verifyDepositBlock(
        uint256 batchNumber, 
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) 
    public 
    operator_only 
    {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        require(batchNumber == lastVerifiedDepositBatch, "must verify batches in order");

        Block storage committed = blocks[blockNumber];
        require(committed.circuit == uint8(Circuit.DEPOSIT), "trying to prove the invalid circuit for this block number");

        DepositBatch storage batch = depositBatches[batchNumber];
        require(batch.blockNumber == blockNumber, "block number in referencing invalid batch number");
        batch.state = uint8(DepositBatchState.VERIFIED);
        batch.timestamp = uint64(block.timestamp);

        // do actual verification
        bool verification_success = verifyProof(
            Circuit.DEPOSIT,
            proof, 
            lastVerifiedRoot, 
            committed.newRoot, 
            committed.publicDataCommitment
        );
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedDepositBatch++;
        lastVerifiedRoot = committed.newRoot;

        uint128 totalFees = clearDepositBatch(batchNumber, accoundIDs);
        committed.totalFees = totalFees;
        balances[committed.prover] += totalFees;
        // process the block information
    }

    // transaction data is trivial: 3 bytes of in-plasma address, 16 bytes of amount and 32 bytes of public key
    function processDepositBlockData(uint256 batchNumber, uint24[DEPOSIT_BATCH_SIZE] memory accountIDs, bytes memory txData) 
    internal 
    view
    {
        uint256 chunk;
        uint256 publicKey;
        uint256 pointer = 32;
        for (uint256 i = 0; i < DEPOSIT_BATCH_SIZE; i++) { 
            // this is a cheap way to ensure that all requests are unique, without O(n) MSTORE
            // it also automatically guarantees that all requests requests from the batch have been executed
            require(i == 0 || accountIDs[i] == 0 || accountIDs[i] > accountIDs[i-1], "accountID are not properly ordered");
            assembly {
                chunk := mload(add(txData, pointer))
            }
            pointer += 19;
            assembly {
                publicKey := mload(add(txData, pointer))
            }
            pointer += 32;
            require(accountIDs[i] == chunk >> 232, "invalid account ID in commitment");
            DepositRequest memory request = depositRequests[batchNumber][accountIDs[i]];
            require(request.amount == chunk << 24 >> 128, "invalid request amount in commitment");
            Account memory accountInfo = accounts[accountIDs[i]];
            require(accountInfo.publicKey == publicKey, "invalid public key in commitment");
        }
    }

    // transaction data is trivial: 3 bytes of in-plasma address and 16 bytes of amount
    function clearDepositBatch(uint256 batchNumber, uint24[DEPOSIT_BATCH_SIZE] memory accountIDs) 
    internal 
    returns (uint128 totalFees)
    {
        uint128 batchFee = depositBatches[batchNumber].batchFee;
        for (uint256 i = 0; i < DEPOSIT_BATCH_SIZE; i++) { 
            if (accountIDs[i] == 0) {
                // this was just a padding
                continue;
            }
            require(i == 0 || accountIDs[i] > accountIDs[i-1], "accountID are not properly ordered");
            DepositRequest storage request = depositRequests[batchNumber][accountIDs[i]];
            require(request.amount != 0, "trying to process an empty request and collect fees");
            delete depositRequests[batchNumber][accountIDs[i]];
            totalFees += batchFee;
        }
        return totalFees;
    }

    function packAndValidatePublicKey(uint256[2] memory publicKey)
    public
    pure
    returns(uint256 packed) {
        // group check + packing
        packed = publicKey[1] | ((publicKey[0] & 1) << 255);
        return packed;
    }

}

contract PlasmaExitor is Plasma {

    uint256 constant EXIT_BATCH_SIZE = 32;
    uint256 totalExitRequests; 
    uint256 lastCommittedExitBatch;
    uint256 lastVerifiedExitBatch;
    uint128 currentExitBatchFee; 

    // batch number => (plasma address => exit flag)
    mapping (uint256 => mapping (uint24 => bool)) public exitRequests;
    mapping (uint256 => ExitBatch) public exitBatches;

    enum ExitBatchState {
        CREATED,
        COMMITTED,
        VERIFIED
    }

    struct ExitBatch {
        uint8 state;
        uint32 blockNumber;
        uint64 timestamp;
        uint128 batchFee;
    }

    event LogExitRequest(uint256 indexed batchNumber, uint24 indexed accountID);
    event LogCancelExitRequest(uint256 indexed batchNumber, uint24 indexed accountID);

    function exit(uint128 maxFee) 
    public 
    {
        require(maxFee <= currentExitBatchFee, "deposit fee is less than required");
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "empty accounts can not exit");

        uint256 currentBatch = totalExitRequests/EXIT_BATCH_SIZE;
        // write aux info about the batch
        ExitBatch storage batch = exitBatches[currentBatch];
        if (batch.timestamp == 0) {
            batch.state = uint8(ExitBatchState.CREATED);
        }
        batch.timestamp = uint64(block.timestamp);
        batch.batchFee = currentExitBatchFee;

        exitRequests[currentBatch][accountID] = true;

        totalExitRequests++;

        emit LogExitRequest(currentBatch, accountID);
    }

    // allow users to cancel a deposit if the work on the next proof is not yet started
    function cancelExit()
    public
    {
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "trying to cancel a deposit for non-existing account");
        uint256 currentBatch = totalExitRequests/EXIT_BATCH_SIZE;
        require(exitRequests[currentBatch][accountID], "exit request should exist");
        emit LogCancelExitRequest(currentBatch, accountID);
        delete exitRequests[currentBatch][accountID];
        totalExitRequests--;
    }

    function startNextExitBatch(uint128 newBatchFee)
    public
    operator_only()
    {
        uint256 currentBatch = totalExitRequests/EXIT_BATCH_SIZE;
        uint256 inTheCurrentBatch = totalExitRequests % EXIT_BATCH_SIZE;
        if (inTheCurrentBatch != 0) {
            totalExitRequests = (currentBatch + 1) * EXIT_BATCH_SIZE;
        }
        currentExitBatchFee = newBatchFee;
    }

    // pure function to calculate commitment formats
    function createPublicDataCommitmentForExit(uint32 blockNumber, bytes memory txDataPacked)
    public 
    pure
    returns (bytes32 h) {

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        
        return finalHash;
    }

    // on commitment to some block we just commit to SOME public data, that will be parsed 
    // ONLY when proof is presented
    
    function commitExitBlock(
        uint256 batchNumber,
        uint32 blockNumber, 
        bytes32 publicDataCommitment,
        bytes32 newRoot
    ) 
    public 
    operator_only 
    {
        require(blockNumber == totalCommitted + 1, "may only commit next block");
        require(batchNumber == lastCommittedExitBatch, "trying to commit batch out of order");
        
        ExitBatch storage batch = exitBatches[batchNumber];
        batch.state = uint8(ExitBatchState.COMMITTED);
        batch.blockNumber = blockNumber;
        batch.timestamp = uint64(block.timestamp);
                
        blocks[blockNumber] = Block(
            uint8(Circuit.WITHDRAWAL), 
            uint64(block.timestamp + MAX_DELAY), 
            0, 
            newRoot, 
            publicDataCommitment, 
            msg.sender
        );
        emit BlockCommitted(blockNumber);
        totalCommitted++;
        lastCommittedExitBatch++;
    }

    // exit block is special - to avoid storge writes an exit data is sent on verification,
    // but not on commitment
    function verifyExitBlock(
        uint256 batchNumber, 
        uint24[EXIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        bytes memory txDataPacked, 
        uint256[8] memory proof
    ) 
    public 
    operator_only 
    {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified + 1, "may only verify next block");
        require(batchNumber == lastVerifiedExitBatch, "trying to prove batch out of order");
        bytes32 publicDataCommitment = createPublicDataCommitmentForExit(blockNumber, txDataPacked);

        Block storage committed = blocks[blockNumber];
        require(committed.circuit == uint8(Circuit.WITHDRAWAL), "trying to prove the invalid circuit for this block number");
        require(committed.publicDataCommitment == publicDataCommitment, "public data is different with a committment");

        ExitBatch storage batch = exitBatches[batchNumber];
        require(batch.blockNumber == blockNumber, "block number in referencing invalid batch number");
        batch.state = uint8(ExitBatchState.VERIFIED);
        batch.timestamp = uint64(block.timestamp);
        uint128 batchFee = batch.batchFee;
        // do actual verification

        bool verification_success = verifyProof(
            Circuit.WITHDRAWAL, 
            proof, 
            lastVerifiedRoot, 
            committed.newRoot, 
            committed.publicDataCommitment
        );
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        totalVerified++;
        lastVerifiedExitBatch++;
        lastVerifiedRoot = committed.newRoot;

        uint128 totalFees = processExitBlockData(batchNumber, batchFee, accoundIDs, txDataPacked);
        committed.totalFees = totalFees;
        balances[committed.prover] += totalFees;
        // process the block information
    }

    // transaction data is trivial: 3 bytes of in-plasma address, 16 bytes of amount
    function processExitBlockData(
        uint256 batchNumber, 
        uint128 batchFee, 
        uint24[EXIT_BATCH_SIZE] memory accountIDs, 
        bytes memory txData
    ) 
    internal 
    returns (uint128 totalFees)
    {
        uint256 chunk;
        uint256 pointer = 32;
        address accountOwner;
        uint128 scaledAmount;
        for (uint256 i = 0; i < EXIT_BATCH_SIZE; i++) { 
            // this is a cheap way to ensure that all requests are unique, without O(n) MSTORE
            // it also automatically guarantees that all requests requests from the batch have been executed
            if (accountIDs[i] == 0) {
                continue;
            }
            require(i == 0 || accountIDs[i] > accountIDs[i-1], "accountID are not properly ordered");
            assembly {
                chunk := mload(add(txData, pointer))
            }
            pointer += 19;
            
            require(accountIDs[i] == chunk >> 232, "invalid account ID in commitment");
            require(exitRequests[batchNumber][accountIDs[i]], "there was not such exit request");
            delete exitRequests[batchNumber][accountIDs[i]];
            totalFees += batchFee;
            accountOwner = accounts[accountIDs[i]].owner;
            scaledAmount = uint128(chunk << 24 >> 128);
            scaledAmount -= batchFee;
            accountOwner.transfer(scale_from(scaledAmount));
        }
    }
}

contract PlasmaContract is PlasmaDepositor, PlasmaExitor {}