pragma solidity ^0.4.24;

import {Plasma} from "./Plasma.sol";

contract PlasmaTransactor is Plasma {
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
}