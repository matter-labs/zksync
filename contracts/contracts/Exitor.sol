pragma solidity ^0.4.24;

import {FranklinCommon} from "./common/FranklinCommon.sol";
import {TwistedEdwards} from "./common/TwistedEdwards.sol";

// interface ExitorInterface {
//     function exit() external payable;
//     function cancelExit() external;
//     function startNextExitBatch() external;
//     function changeExitBatchFee(uint128 newBatchFee) external;
//     function commitExitBlock(
//         uint256 batchNumber,
//         uint24[EXIT_BATCH_SIZE] accoundIDs, 
//         uint32 blockNumber, 
//         bytes txDataPacked, 
//         bytes32 newRoot
//     ) external;
//     function verifyExitBlock(
//         uint256 batchNumber, 
//         uint32 blockNumber, 
//         uint256[8] proof
//     ) external;
//     function withdrawUserBalance(
//         uint256 iterationsLimit
//     ) external;
// }

// this procedure is a one-time full exit with a removal 
// of the public key from the tree
contract Exitor is FranklinCommon {

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
        uint32 tail;
        for (uint256 i = 0; i < EXIT_BATCH_SIZE; i++) { 
            // this is a cheap way to ensure that all requests are unique, without O(n) MSTORE
            // it also automatically guarantees that all requests requests from the batch have been executed
            if (accountIDs[i] == 0) {
                continue;
            }
            require(i == 0 || accountIDs[i] > accountIDs[i-1], "accountIDs are not properly ordered");
            assembly {
                chunk := mload(add(txData, pointer))
            }
            pointer += 19;
            
            require(accountIDs[i] == chunk >> 232, "invalid account ID in commitment");
            Account storage account = accounts[accountIDs[i]];
            require(account.state == uint8(AccountState.PENDING_EXIT), "there was no such exit request");
            require(account.exitBatchNumber == uint32(batchNumber), "account was registered for exit in another batch");

            // accountOwner = accounts[accountIDs[i]].owner;
            scaledAmount = uint128(chunk << 24 >> 128);
   
            ExitLeaf memory newLeaf;
            tail = account.exitListTail;
            if (tail == 0) {
                // create a fresh list that is both head and tail
                newLeaf = ExitLeaf(0, scaledAmount);
                exitLeafs[account.owner][blockNumber] = newLeaf;
                account.exitListTail = blockNumber;
            } else {
                // previous tail is somewhere in the past
                ExitLeaf storage previousExitLeaf = exitLeafs[account.owner][tail];

                newLeaf = ExitLeaf(0, scaledAmount);
                previousExitLeaf.nextID = blockNumber;

                exitLeafs[account.owner][blockNumber] = newLeaf;
                account.exitListTail = blockNumber;
            }

            // if there was no head - point to here
            if (account.exitListHead == 0) {
                account.exitListHead = blockNumber;
            }

            // exitAmounts[accountOwner][blockNumber] = scaledAmount;
            account.state = uint8(AccountState.UNCONFIRMED_EXIT);

            emit LogCompleteExit(accountOwner, blockNumber, accountIDs[i]);
        }
    }

    // function withdrawUserBalance(
    //     uint32[] blockNumbers
    // )
    // public
    // {
    //     require(blockNumbers.length > 0, "requires non-empty set");
    //     uint256 totalAmountInWei;
    //     uint32 blockNumber;
    //     for (uint256 i = 0; i < blockNumbers.length; i++) {
    //         blockNumber = blockNumbers[i]; 

    //         require(blockNumber <= lastVerifiedBlockNumber, "can only process exits from verified blocks");
    //         uint24 accountID = ethereumAddressToAccountID[msg.sender];
    //         uint128 balance;
    //         uint256 amountInWei;
    //         if (accountID != 0) {
    //             // user either didn't fully exit or didn't take full exit balance yet
    //             Account storage account = accounts[accountID];
    //             if (account.state == uint8(AccountState.UNCONFIRMED_EXIT)) {
    //                 uint256 batchNumber = account.exitBatchNumber;
    //                 ExitBatch storage batch = exitBatches[batchNumber];
    //                 if (blockNumber == batch.blockNumber) {
    //                     balance = exitAmounts[msg.sender][blockNumber];

    //                     delete accounts[accountID];
    //                     delete ethereumAddressToAccountID[msg.sender];
    //                     delete exitAmounts[msg.sender][blockNumber];

    //                     amountInWei = scaleFromPlasmaUnitsIntoWei(balance);
    //                     totalAmountInWei += amountInWei;
    //                     continue;
    //                 }
    //             }
    //         }
    //         // user account information is already deleted or it's not the block number where a full exit has happened
    //         // we require a non-zero balance in this case cause chain cleanup is not required
    //         balance = exitAmounts[msg.sender][blockNumber];

    //         require(balance != 0, "nothing to exit");
    //         delete exitAmounts[msg.sender][blockNumber];

    //         amountInWei = scaleFromPlasmaUnitsIntoWei(balance);
    //         totalAmountInWei += amountInWei;
    //     }
    //     msg.sender.transfer(totalAmountInWei);
    // }

    function withdrawUserBalance(
        uint256 iterationsLimit
    )
    public
    {
        require(iterationsLimit > 0, "must iterate");
        uint256 totalAmountInWei;
        uint256 amountInWei;
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        require(accountID != 0, "this should not happen as exiting happens one by one until the complete one");
        Account storage account = accounts[accountID];

        uint32 currentHead = account.exitListHead;
        uint32 nextHead = currentHead;

        for (uint256 i = 0; i < iterationsLimit; i++) {
            if (currentHead > lastVerifiedBlockNumber) {
                if (i == 0) {
                    revert("nothing to process");
                } else {
                    return;
                }
            }
            ExitLeaf storage leaf = exitLeafs[msg.sender][currentHead];

            amountInWei = scaleFromPlasmaUnitsIntoWei(leaf.amount);
            totalAmountInWei += amountInWei;

            // no matter if the next leafID is empty or not we can assign it

            nextHead = leaf.nextID;
            delete exitLeafs[msg.sender][currentHead];

            if (nextHead == 0 && account.state == uint8(AccountState.UNCONFIRMED_EXIT)) {
                // there is no next element AND account is exiting, so it must be the complete exit leaf
                uint256 batchNumber = account.exitBatchNumber;
                ExitBatch storage batch = exitBatches[batchNumber];
                require(currentHead == batch.blockNumber, "last item in the list should match the complete exit block");
                delete accounts[accountID];
                delete ethereumAddressToAccountID[msg.sender];
            }

            if (nextHead == 0) {
                // this is an end of the list
                break;
            } else {
                currentHead = nextHead;
            }

        }

        account.exitListHead = nextHead;
        if (nextHead == 0) {
            account.exitListTail = 0;
        }

        msg.sender.transfer(totalAmountInWei);
    }

}