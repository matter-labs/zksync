pragma solidity ^0.5.0;

import "./Verifier.sol";
import "./VerificationKeys.sol";


contract Plasma is Verifier, VerificationKeys {

    uint32 constant DEADLINE = 0; // seconds, to define

    enum Circuit {
        DEPOSIT,
        UPDATE,
        WITHDRAWAL
    }

    struct Block {
        Circuit circuit;

        uint128 totalFees;
        bytes32 newRoot;
        bytes32 finalHash;

        // TODO: Everybody should be able to provide proof and collect fees when deadline is crossed
        address prover;
        uint32  deadline;
    }

    bytes32 public lastVerifiedRoot;

    // Key is block number
    mapping (uint256 => Block) public blocks;

    uint256 public totalCommitted;
    uint256 public totalVerified;

    // Balances for distributing fees to provers
    mapping (address => uint256) public balance;

    // Public API

    constructor(bytes32 _initialRoot) public {
        lastVerifiedRoot = _initialRoot;
    }

    function commitBlock(uint32 blockNumber, uint128 totalFees, bytes memory txDataPacked, bytes32 newRoot) public {
        require(blockNumber == totalCommitted, "may only commit next block");

        bytes32 initialHash = sha256(abi.encodePacked(uint256(blockNumber), uint256(totalFees)));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));

        // TODO: need a strategy to avoid front-running msg.sender
        blocks[totalCommitted] = Block(Circuit.UPDATE, totalFees, newRoot, finalHash, msg.sender, uint32(now + DEADLINE));
        totalCommitted++;
    }

    function verifyBlock(uint32 blockNumber, uint256[8] memory proof) public {
        require(totalVerified < totalCommitted, "no committed block to verify");
        require(blockNumber == totalVerified, "may only verified next block");
        Block memory committed = blocks[blockNumber];
        require(verifyUpdateProof(proof, lastVerifiedRoot, committed.newRoot, committed.finalHash), "invalid proof");
        totalVerified++;
        lastVerifiedRoot = committed.newRoot;

        // TODO: how to deal with deadline? Penalties?
        balance[committed.prover] += committed.totalFees;
    }

    // Implementation

    function verifyUpdateProof(uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVkUpdateCircuit();
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash);
        return Verify(vk, gammaABC, proof, inputs);
    }

}
