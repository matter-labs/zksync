pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";

// this procedure is a one-time full exit with a removal 
// of the public key from the tree
contract PlasmaExitor is Plasma {

    function exit() 
    public 
    payable
    {
        uint128 userFee = scaleIntoPlasmaUnitsFromWei(msg.value);
        require(userFee >= currentExitBatchFee, "exit fee should be more than required by the operator");
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

        Account storage account = accounts[accountID];
        require(account.state == uint8(AccountState.REGISTERED), "only accounts that are registered and not pending exit can exit");
        account.state = uint8(AccountState.PENDING_EXIT);
        account.exitBatchNumber = uint32(currentBatch);

        totalExitRequests++;

        emit LogExitRequest(currentBatch, accountID);
    }

    // allow users to cancel an exit if the work on the next proof is not yet started
    function cancelExit()
    public
    {
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "trying to cancel a deposit for non-existing account");
        uint256 currentBatch = totalExitRequests/EXIT_BATCH_SIZE;
        uint256 requestsInThisBatch = totalExitRequests % EXIT_BATCH_SIZE;

        // if the first request in a batch is canceled - clear it to stop the countdown
        if (requestsInThisBatch == 0) {
            delete exitBatches[currentBatch];
        }

        Account storage account = accounts[accountID];
        require(account.state == uint8(AccountState.PENDING_EXIT), "can only cancel exits for accounts that are pending exit");
        require(account.exitBatchNumber == uint32(currentBatch), "can not cancel an exit in the batch that was already accepted");
        account.state = uint8(AccountState.REGISTERED);
        account.exitBatchNumber = 0;

        emit LogCancelExitRequest(currentBatch, accountID);

        totalExitRequests--;

        // TODO may be return an fee that was collected
    }

    // this does not work in multi-operator mode
    function startNextExitBatch()
    public
    operator_only()
    {
        uint256 currentBatch = totalExitRequests/EXIT_BATCH_SIZE;
        uint256 inTheCurrentBatch = totalExitRequests % EXIT_BATCH_SIZE;
        if (inTheCurrentBatch != 0) {
            totalExitRequests = (currentBatch + 1) * EXIT_BATCH_SIZE;
        } else {
            revert("it's not necessary to bump the batch number");
        }
    }

    // this does not work in multi-operator mode
    function changeExitBatchFee(uint128 newBatchFee)
    public
    operator_only()
    {
        if (currentExitBatchFee == 0) {
            revert("fee is already at minimum");
        }
        if (newBatchFee < currentExitBatchFee) {
            currentExitBatchFee = newBatchFee;
        } else {
            revert("can not increase an exit fee");
        }
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
        uint24[EXIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) 
    public 
    operator_only 
    {
        require(blockNumber == lastCommittedBlockNumber + 1, "may only commit next block");
        require(batchNumber == lastCommittedExitBatch, "trying to commit batch out of order");
        
        ExitBatch storage batch = exitBatches[batchNumber];
        batch.state = uint8(ExitBatchState.COMMITTED);
        batch.blockNumber = blockNumber;
        batch.timestamp = uint64(block.timestamp);
                
        bytes32 publicDataCommitment = createPublicDataCommitmentForExit(blockNumber, txDataPacked);

        blocks[blockNumber] = Block(
            uint8(Circuit.EXIT), 
            uint64(block.timestamp + MAX_DELAY), 
            0, 
            newRoot, 
            publicDataCommitment, 
            msg.sender
        );
        emit BlockCommitted(blockNumber);
        lastCommittedBlockNumber++;
        lastCommittedExitBatch++;

        // process the block information
        processExitBlockData(batchNumber, blockNumber, accoundIDs, txDataPacked);
    }

    // exit block is special - to avoid storge writes an exit data is sent on verification,
    // but not on commitment
    function verifyExitBlock(
        uint256 batchNumber, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) 
    public 
    operator_only 
    {
        require(lastVerifiedBlockNumber < lastCommittedBlockNumber, "no committed block to verify");
        require(blockNumber == lastVerifiedBlockNumber + 1, "may only verify next block");
        require(batchNumber == lastVerifiedExitBatch, "trying to prove batch out of order");

        Block storage committed = blocks[blockNumber];
        require(committed.circuit == uint8(Circuit.EXIT), "trying to prove the invalid circuit for this block number");

        ExitBatch storage batch = exitBatches[batchNumber];
        require(batch.blockNumber == blockNumber, "block number in referencing invalid batch number");
        batch.state = uint8(ExitBatchState.VERIFIED);
        batch.timestamp = uint64(block.timestamp);
        // do actual verification

        bool verification_success = verifyProof(
            Circuit.EXIT, 
            proof, 
            lastVerifiedRoot, 
            committed.newRoot, 
            committed.publicDataCommitment
        );
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        lastVerifiedBlockNumber++;
        lastVerifiedExitBatch++;
        lastVerifiedRoot = committed.newRoot;
    }

    // transaction data is trivial: 3 bytes of in-plasma address, 16 bytes of amount
    // same a for partial exits - write to storage, so users can pull the balances later
    function processExitBlockData(
        uint256 batchNumber, 
        uint32 blockNumber,
        uint24[EXIT_BATCH_SIZE] memory accountIDs, 
        bytes memory txData
    ) 
    internal 
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
            Account storage account = accounts[accountIDs[i]];
            require(account.state == uint8(AccountState.PENDING_EXIT), "there was not such exit request");
            require(account.exitBatchNumber == uint32(batchNumber), "account was registered for exit in another batch");

            accountOwner = accounts[accountIDs[i]].owner;
            scaledAmount = uint128(chunk << 24 >> 128);
            fullExits[blockNumber][accountIDs[i]] = scaledAmount;

            accountOwner.transfer(scaleFromPlasmaUnitsIntoWei(scaledAmount));
        }
    }

    function withdrawFullExitBalance(
        uint32 blockNumber
    )
    public
    {
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "trying to access a non-existent account");
        require(blockNumber <= lastVerifiedBlockNumber, "can only process exits from verified blocks");
        uint128 balance = fullExits[blockNumber][accountID];
        require(balance != 0, "nothing to exit");
        delete fullExits[blockNumber][accountID];
        uint256 amountInWei = scaleFromPlasmaUnitsIntoWei(balance);
        delete accounts[accountID];
        delete ethereumAddressToAccountID[msg.sender];
        msg.sender.transfer(amountInWei);
    }
}