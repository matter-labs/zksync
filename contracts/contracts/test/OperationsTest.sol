pragma solidity 0.5.16;

import "../Operations.sol";


contract OperationsTest {

    function testDeposit() external pure {
        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 0x0102,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5
        });

        bytes memory pubdata = Operations.writeDepositPubdata(x);
        Operations.Deposit memory r = Operations.readDepositPubdata(pubdata, 0);

        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.amount == r.amount,   "amount mismatch");
        require(x.owner == r.owner,     "owner mismatch");
    }

    function testFullExit() external pure{
        bytes memory hash = new bytes(20);
        Operations.FullExit memory x = Operations.FullExit({
            pubkeyHash: hash,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            tokenId: 0x3132,
            nonce: 0x41424344
        });

        bytes memory pubdata = Operations.writeFullExitPubdata(x);
        Operations.FullExit memory r = Operations.readFullExitPubdata(pubdata, 0);

        require(x.owner == r.owner,     "owner mismatch");
        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.nonce == r.nonce,     "nonce mismatch");
        require(keccak256(x.pubkeyHash) == keccak256(r.pubkeyHash), "hash mismatch");
    }

    function testPartialExit() external pure{
        Operations.PartialExit memory x = Operations.PartialExit({
            tokenId: 0x3132,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5
        });

        bytes memory pubdata = Operations.writePartialExitPubdata(x);
        Operations.PartialExit memory r = Operations.readPartialExitPubdata(pubdata, 0);

        require(x.owner == r.owner,     "owner mismatch");
        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.amount == r.amount,   "amount mismatch");
    }
    
}

