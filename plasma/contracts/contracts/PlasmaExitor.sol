pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";

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
        // batch countdown start from the first request
        if (batch.timestamp == 0) {
            batch.state = uint8(ExitBatchState.CREATED);
            batch.timestamp = uint64(block.timestamp);
            batch.batchFee = currentExitBatchFee;
        }
        
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
        uint256 requestsInThisBatch = totalExitRequests * EXIT_BATCH_SIZE;
        require(exitRequests[currentBatch][accountID], "exit request should exist");
        emit LogCancelExitRequest(currentBatch, accountID);
        // if the first request in a batch is canceled - clear it to stop the countdown
        if (requestsInThisBatch == 0) {
            delete exitBatches[currentBatch];
        }
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
            accountOwner.transfer(scaleFromPlasmaUnitsIntoWei(scaledAmount));
        }
    }
}