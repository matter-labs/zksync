pragma solidity ^0.4.24;

import {FranklinCommon} from "./common/FranklinCommon.sol";
import {TwistedEdwards} from "./common/TwistedEdwards.sol";

// interface DepositorInterface {
//     function deposit(uint256[2] publicKey, uint128 maxFee) external payable;
//     function depositInto(uint24 accountID, uint128 maxFee) external payable;
//     function cancelDeposit() external;
//     function startNextDepositBatch() external;
//     function changeDepositBatchFee(uint128 newBatchFee) external;
//     function commitDepositBlock(
//         uint256 batchNumber,
//         uint24[DEPOSIT_BATCH_SIZE] accoundIDs,
//         uint32 blockNumber, 
//         bytes32 newRoot
//     ) external;
    
//     function verifyDepositBlock(
//         uint256 batchNumber, 
//         uint24[DEPOSIT_BATCH_SIZE] accoundIDs, 
//         uint32 blockNumber, 
//         uint256[8] proof
//     ) external; 
// }

contract Depositor is FranklinCommon {
    
    function deposit(uint256[2] memory publicKey, uint128 maxFee) 
    public 
    payable 
    active_only()
    {
        // only registed an account or do the lookup
        uint24 accountID = ethereumAddressToAccountID[msg.sender];
        if (accountID == 0) {
            // register new account
            uint256 packedKey = packAndValidatePublicKey(publicKey);
            ethereumAddressToAccountID[msg.sender] = nextAccountToRegister;
            Account memory freshAccount = Account(
                uint8(AccountState.REGISTERED),
                uint32(0),
                msg.sender,
                packedKey,
                uint32(0),
                uint32(0)
            );
            accountID = nextAccountToRegister;
            accounts[accountID] = freshAccount;
            // bump accounts counter
            nextAccountToRegister += 1;
        }
        depositInto(accountID, maxFee);
    }

    function depositInto(uint24 accountID, uint128 maxFee) 
    public 
    payable 
    active_only()
    {
        // this comparison is to avoid frontrunning between user
        // and the operator
        require(maxFee >= currentDepositBatchFee, "deposit fee is less than required");
        uint128 scaledValue = scaleIntoPlasmaUnitsFromWei(msg.value);
        require(scaledValue >= currentDepositBatchFee, "deposit amount should cover the fee");
        require(accountID < nextAccountToRegister, "for now only allow to deposit into non-empty accounts");
        // read account info
        Account memory accountInformation = accounts[accountID];
        require(accountInformation.state == uint8(AccountState.REGISTERED), "can only deposit into registered account");

        // work with a deposit
        uint256 currentBatch = totalDepositRequests / DEPOSIT_BATCH_SIZE;
        // write aux info about the batch
        DepositBatch storage batch = depositBatches[currentBatch];
        // amount of time for an operator to process a batch is counted
        // from the first deposit in the batch
        if (batch.timestamp == 0) {
            batch.state = uint8(DepositBatchState.CREATED);
            batch.numRequests = uint24(0);
            batch.timestamp = uint64(block.timestamp);
            batch.batchFee = currentDepositBatchFee;
        }
        scaledValue -= currentDepositBatchFee;
        // get request in this batch for this account
        DepositRequest storage request = depositRequests[currentBatch][accountID];
        
        if (request.amount == 0) {
            // this is a new request in this batch
            batch.numRequests++;
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
        uint256 requestsInThisBatch = totalDepositRequests % DEPOSIT_BATCH_SIZE;
        DepositBatch storage batch = depositBatches[currentBatch];
        // this check is most likely excessive, 
        require(batch.state == uint8(DepositBatchState.CREATED), "canceling is only allowed for batches that are not yet committed");
    
        DepositRequest storage request = depositRequests[currentBatch][accountID];
        uint128 depositAmount = request.amount;
        require(depositAmount > 0, "trying to cancel an empty deposit");

        // add a batch fee that was previously subtracted
        depositAmount += batch.batchFee;
        // log and clear the storage
        emit LogCancelDepositRequest(currentBatch, accountID);
        // if the first request in a batch is canceled - clear it to stop the countdown
        if (requestsInThisBatch == 0) { 
            delete depositBatches[currentBatch];
        }
        delete depositRequests[currentBatch][accountID];
        totalDepositRequests--;
        batch.numRequests--;

        msg.sender.transfer(scaleFromPlasmaUnitsIntoWei(depositAmount));
    }

    function startNextDepositBatch()
    public
    active_only()
    operator_only()
    {
        uint256 currentBatch = totalDepositRequests/DEPOSIT_BATCH_SIZE;
        uint256 inTheCurrentBatch = totalDepositRequests % DEPOSIT_BATCH_SIZE;
        if (inTheCurrentBatch != 0) {
            totalDepositRequests = (currentBatch + 1) * DEPOSIT_BATCH_SIZE;
        } else {
            revert("it's not necessary to bump the batch number");
        }
 
    }

    function changeDepositBatchFee(uint128 newBatchFee)
    public
    active_only()
    operator_only()
    {
        if (currentDepositBatchFee != newBatchFee) {
            currentDepositBatchFee = newBatchFee;
        } else {
            revert("fee adjustment makes no sense");
        } 
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
        bytes32 newRoot
    ) 
    public
    active_only()
    operator_only()
    {
        require(blockNumber == lastCommittedBlockNumber + 1, "may only commit next block");
        require(batchNumber == lastCommittedDepositBatch, "trying to commit batch out of order");
        
        DepositBatch storage batch = depositBatches[batchNumber];
        batch.state = uint8(DepositBatchState.COMMITTED);
        batch.blockNumber = blockNumber;
        batch.timestamp = uint64(block.timestamp);

        // pack the public data using information that it's already on-chain
        bytes memory txDataPacked = processDepositBlockData(batchNumber, accoundIDs);
        
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
        lastCommittedBlockNumber++;
        lastCommittedDepositBatch++;
    }

    function verifyDepositBlock(
        uint256 batchNumber, 
        uint24[DEPOSIT_BATCH_SIZE] memory accoundIDs, 
        uint32 blockNumber, 
        uint256[8] memory proof
    ) 
    public 
    active_only()
    operator_only()
    {
        require(lastVerifiedBlockNumber < lastCommittedBlockNumber, "no committed block to verify");
        require(blockNumber == lastVerifiedBlockNumber + 1, "may only verify next block");
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
        lastVerifiedBlockNumber++;
        lastVerifiedDepositBatch++;
        lastVerifiedRoot = committed.newRoot;

        uint128 totalFees = clearDepositBatch(batchNumber, accoundIDs);
        committed.totalFees = totalFees;
        balances[committed.prover] += totalFees;
        // process the block information
    }

    // transaction data is trivial: 3 bytes of in-plasma address, 16 bytes of amount and 32 bytes of public key
    function processDepositBlockData(uint256 batchNumber, uint24[DEPOSIT_BATCH_SIZE] memory accountIDs) 
    internal 
    view
    returns (bytes memory txData)
    {
        txData = new bytes(DEPOSIT_BATCH_SIZE * 51);
        uint256 chunk;
        uint128 requestAmount;
        uint256 publicKey;
        uint256 pointer = 32;
        uint24 specialAccountID = SPECIAL_ACCOUNT_DEPOSITS;
        uint256 numRequestsInBatch = uint256(depositBatches[batchNumber].numRequests);
        uint24 id;
        for (uint256 i = 0; i < numRequestsInBatch; i++) { 
            // this is a cheap way to ensure that all requests are unique, without O(n) MSTORE
            // it also automatically guarantees that all requests requests from the batch have been executed
            require(i == 0 || accountIDs[i] > accountIDs[i-1], "accountID are not properly ordered");
            id = accountIDs[i];
            require(id != specialAccountID, "batch should contain non-padding accounts first");
            requestAmount = depositRequests[batchNumber][id].amount;
            publicKey = accounts[id].publicKey;
            // put address and amount into the top bits of the chunk
            // address || amount || 0000...0000
            chunk = ((uint256(id) << 128) + uint256(requestAmount)) << 104;
            // and store it into place
            assembly {
                mstore(add(txData, pointer), chunk)
            }
            pointer += 19;
            assembly {
                mstore(add(txData, pointer), publicKey)
            }
            pointer += 32;
        }
        chunk = uint256(specialAccountID) << 232;
        publicKey = accounts[specialAccountID].publicKey;

        for (i = numRequestsInBatch; i < DEPOSIT_BATCH_SIZE; i++) { 
            id = accountIDs[i];
            require(id == specialAccountID, "padding should be done with special account number");
            assembly {
                mstore(add(txData, pointer), chunk)
            }
            pointer += 19;
            assembly {
                mstore(add(txData, pointer), publicKey)
            }
            pointer += 32;
        }

        return txData;
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
        require(TwistedEdwards.checkOnCurve(publicKey), "public key must be on the curve");
        // group check + packing
        packed = publicKey[1] + ((publicKey[0] & 1) << 255);
        return packed;
    }

    // function () external payable {
    //     address callee = transactor;
    //     assembly {
    //         let memoryPointer := mload(0x40)
    //         calldatacopy(memoryPointer, 0, calldatasize)
    //         let newFreeMemoryPointer := add(memoryPointer, calldatasize)
    //         mstore(0x40, newFreeMemoryPointer)
    //         let retVal := delegatecall(sub(gas, 2000), callee, memoryPointer, calldatasize, newFreeMemoryPointer, 0x40)
    //         let retDataSize := returndatasize
    //         returndatacopy(newFreeMemoryPointer, 0, retDataSize)
    //         switch retVal case 0 { revert(0,0) } default { return(newFreeMemoryPointer, retDataSize) }
    //     }
    // }
}