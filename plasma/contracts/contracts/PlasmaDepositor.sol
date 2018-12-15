pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";

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
        // only registed an account or do the lookup
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
        depositInto(accountID, maxFee);
    }

    function depositInto(uint24 accountID, uint128 maxFee) 
    public 
    payable 
    {
        require(maxFee <= currentDepositBatchFee, "deposit fee is less than required");
        uint128 scaledValue = scaleIntoPlasmaUnitsFromWei(msg.value);
        require(scaledValue > currentDepositBatchFee, "deposit amount should cover the fee");
        require(accountID < nextAccountToRegister, "for now only allow to deposit into non-empty accounts");
        // read account info
        Account memory accountInformation = accounts[accountID];

        // work with a deposit
        uint256 currentBatch = totalDepositRequests/DEPOSIT_BATCH_SIZE;
        // write aux info about the batch
        DepositBatch storage batch = depositBatches[currentBatch];
        // amount of time for an operator to process a batch is counted
        // from the first deposit in the batch
        if (batch.timestamp == 0) {
            batch.state = uint8(DepositBatchState.CREATED);
            batch.timestamp = uint64(block.timestamp);
            batch.batchFee = currentDepositBatchFee;
        }
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
        uint256 requestsInThisBatch = totalDepositRequests * DEPOSIT_BATCH_SIZE;
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

        msg.sender.transfer(scaleFromPlasmaUnitsIntoWei(depositAmount));
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
        for (uint256 i = 0; i < DEPOSIT_BATCH_SIZE; i++) { 
            // this is a cheap way to ensure that all requests are unique, without O(n) MSTORE
            // it also automatically guarantees that all requests requests from the batch have been executed
            require(i == 0 || accountIDs[i] == 0 || accountIDs[i] > accountIDs[i-1], "accountID are not properly ordered");
            requestAmount = depositRequests[batchNumber][accountIDs[i]].amount;
            publicKey = accounts[accountIDs[i]].publicKey;
            // put address and amount into the top bits of the chunk
            // address || amount || 0000...0000
            chunk = ((uint256(accountIDs[i]) << 128) + uint256(requestAmount)) << 104;
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
        // group check + packing
        packed = publicKey[1] | ((publicKey[0] & 1) << 255);
        return packed;
    }

}