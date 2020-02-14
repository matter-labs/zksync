pragma solidity 0.5.16;

import "../Operations.sol";


contract OperationsTest {

    function testDeposit() external pure returns (uint, uint) {
        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 0x0102,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5
        });

        bytes memory pubdata = Operations.writeDepositPubdata(x);
        //require(pubdata.length == Operations.PackedFullExitPubdataBytes());
        (uint offset, Operations.Deposit memory r) = Operations.readDepositPubdata(pubdata, 0);
        require(offset == pubdata.length, "incorrect offset");

        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.amount == r.amount,   "amount mismatch");
        require(x.owner == r.owner,     "owner mismatch");
    }

    function testDepositMatch(bytes calldata offchain) external pure returns (bool) {
        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 0x0102,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5
        });
        bytes memory onchain = Operations.writeDepositPubdata(x);

        return Operations.depositPubdataMatch(onchain, offchain);
    }

    function testFullExit() external pure {
        Operations.FullExit memory x = Operations.FullExit({
            accountId:  0x010203,
            owner:      0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            tokenId:    0x3132,
            amount:     0x101112131415161718191a1b1c1d1e1f
        });

        bytes memory pubdata = Operations.writeFullExitPubdata(x);
        //require(pubdata.length == Operations.PackedDepositPubdataBytes());
        Operations.FullExit memory r = Operations.readFullExitPubdata(pubdata, 0);

        require(x.accountId == r.accountId, "accountId mismatch");
        require(x.owner == r.owner,         "owner mismatch");
        require(x.tokenId == r.tokenId,     "tokenId mismatch");
        require(x.amount == r.amount,       "amount mismatch");
    }

    function testFullExitMatch(bytes calldata offchain) external pure returns (bool) {
        Operations.FullExit memory x = Operations.FullExit({
            accountId:  0x010203,
            owner:      0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            tokenId:    0x3132,
            amount:     0
        });
        bytes memory onchain = Operations.writeFullExitPubdata(x);

        return Operations.fullExitPubdataMatch(onchain, offchain);
    }

    function testPartialExit() external pure {
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

