pragma solidity ^0.4.24;

import {FranklinCommon} from "./common/FranklinCommon.sol";
import {TwistedEdwards} from "./common/TwistedEdwards.sol";

// interface TransactorInterface {
//     function commitTransferBlock(
//         uint32 blockNumber, 
//         uint128 totalFees, 
//         bytes txDataPacked, 
//         bytes32 newRoot
//     ) external;

//     function verifyTransferBlock(uint32 blockNumber, uint256[8] proof) external;
// }

contract Transactor is FranklinCommon {

    function commitTransferBlock(
        uint32 blockNumber, 
        uint128 totalFees, 
        bytes memory txDataPacked, 
        bytes32 newRoot
    ) 
    public 
    operator_only 
    {
        require(blockNumber == lastCommittedBlockNumber + 1, "may only commit next block");

        // create now commitments and write to storage
        bytes32 publicDataCommitment = createPublicDataCommitmentForTransfer(blockNumber, totalFees, txDataPacked);

        blocks[blockNumber] = Block(
            uint8(Circuit.TRANSFER), 
            uint64(block.timestamp + DEADLINE), 
            totalFees, 
            newRoot, 
            publicDataCommitment, 
            msg.sender
        );
        emit BlockCommitted(blockNumber);
        parsePartialExitsBlock(blockNumber, txDataPacked);
        lastCommittedBlockNumber++;
    }

    function verifyTransferBlock(uint32 blockNumber, uint256[8] memory proof) public operator_only {
        require(lastVerifiedBlockNumber < lastCommittedBlockNumber, "no committed block to verify");
        require(blockNumber == lastVerifiedBlockNumber + 1, "may only verify next block");
        Block memory committed = blocks[blockNumber];
        require(committed.circuit == uint8(Circuit.TRANSFER), "trying to prove the invalid circuit for this block number");
        bool verification_success = verifyProof(
            Circuit.TRANSFER, 
            proof, 
            lastVerifiedRoot, 
            committed.newRoot, 
            committed.publicDataCommitment
        );
        require(verification_success, "invalid proof");

        emit BlockVerified(blockNumber);
        lastVerifiedBlockNumber++;
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

        // // this can be used if inside of a SNARK the edge case of transfer 
        // // from 0 to 0 with zero amount and fee
        // // is properly covered. Account number 0 does NOT have a public key
        // if (txDataPacked.length / 9 == TRANSFER_BLOCK_SIZE) {
        //     bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));
        // } else {
        //     // do the ad-hoc padding with zeroes
        //     bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked, new bytes(TRANSFER_BLOCK_SIZE * 9 - txDataPacked.length)));
        // }
        
        return finalHash;
    }

    // parse every tx in a block and of destination == 0 - write a partial exit information
    function parsePartialExitsBlock(
        uint32 blockNumber,
        bytes memory txDataPacked
    )
    internal
    {
        uint256 chunk;
        uint256 pointer = 32;
        uint24 to;
        uint24 from;
        uint128 scaledAmount;
        uint16 floatValue;
        // there is no check for a length of the public data because it's not provable if broken
        // unless sha256 collision is found
        for (uint256 i = 0; i < txDataPacked.length / 9; i++) { 
            assembly {
                chunk := mload(add(txDataPacked, pointer))
            }
            pointer += 9;
            to = uint24((chunk << 24) >> 232);
            if (to == 0) {
                from = uint24(chunk >> 232);
                if (from == 0) {
                    continue;
                }
                floatValue = uint16((chunk << 48) >> 240);
                Account storage account = accounts[from];
                if (account.owner == address(0)) {
                    continue;
                }

                scaledAmount = parseFloat(floatValue);

                ExitLeaf memory newLeaf;
                if (account.exitListTail == 0) {
                    // create a fresh list that is both head and tail
                    newLeaf = ExitLeaf(0, scaledAmount);
                    exitLeafs[account.owner][blockNumber] = newLeaf;
                    account.exitListTail = blockNumber;
                } else {
                    // such global "else" is intentional, otherwise account.exitListTail == blockNumber will
                    // happen after the assignment above
                    if (account.exitListTail == blockNumber) {
                        // to exits in the same block, happens
                        ExitLeaf storage thisExitLeaf = exitLeafs[account.owner][account.exitListTail];
                        thisExitLeaf.amount += scaledAmount;
                    } else {
                        // previous tail is somewhere in the past
                        ExitLeaf storage previousExitLeaf = exitLeafs[account.owner][account.exitListTail];
                        newLeaf = ExitLeaf(0, scaledAmount);
                        previousExitLeaf.nextID = blockNumber;

                        exitLeafs[account.owner][blockNumber] = newLeaf;
                        account.exitListTail = blockNumber;
                    }
                }

                // if there was no head - point to here
                if (account.exitListHead == 0) {
                    account.exitListHead = blockNumber;
                }

                // exitAmounts[userAddress][blockNumber] += scaledAmount;
                emit LogExit(account.owner, blockNumber);
            }
        }
    }

    // parses 5 bits of exponent base 10 and 11 bits of mantissa
    // there are no overflow checks here cause maximum float value < UINT128_MAX
    function parseFloat(
        uint16 float  
    )
    public 
    pure
    returns (uint128 scaledValue)
    {
        uint128 exponent = 0;
        uint128 powerOfTwo = 1;
        for (uint256 i = 0; i < 5; i++) {
            if (float & (1 << (15 - i)) > 0) {
                exponent += powerOfTwo;
            }
            powerOfTwo = powerOfTwo * 2;
        }
        exponent = uint128(10) ** exponent;

        uint128 mantissa = 0;
        powerOfTwo = 1;
        // TODO: change when 0.5.0 is used
        for (i = 0; i < 11; i++) {
            if (float & (1 << (10 - i)) > 0) {
                mantissa += powerOfTwo;
            }
            powerOfTwo = powerOfTwo * 2;
        }
        return exponent * mantissa;
    }

    // function () external payable {
    //     address callee = exitor;
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