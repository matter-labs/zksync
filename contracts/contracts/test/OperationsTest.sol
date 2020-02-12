pragma solidity 0.5.16;

import "../Operations.sol";


contract OperationsTest {

    function testDeposit() external pure returns (uint, uint) {
        bytes memory pubkey = new bytes(20);
        pubkey[0] = 0x01;
        pubkey[19] = 0x02;

        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 0x0102,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            pubkeyHash: pubkey
        });

        bytes memory pubdata = Operations.writeDepositPubdata(x);
        (uint offset, Operations.Deposit memory r) = Operations.readDepositPubdata(pubdata, 0);

        require(offset == pubdata.length, "incorrect offset");
        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.amount == r.amount,   "amount mismatch");
        require(x.owner == r.owner,     "owner mismatch");
        require(keccak256(x.pubkeyHash) == keccak256(pubkey), "pubkey hash mismatch");
    }

    function testDepositMatch(bytes calldata offchain) external pure returns (bool) {
        bytes memory pubkey = new bytes(20);
        pubkey[0] = 0x01;
        pubkey[19] = 0x02;
        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 0x0102,
            amount: 0x101112131415161718191a1b1c1d1e1f,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            pubkeyHash: pubkey
        });
        bytes memory onchain = Operations.writeDepositPubdata(x);

        return Operations.depositPubdataMatch(onchain, offchain);
    }

    function testFullExit() external pure {
        bytes memory pubkey = new bytes(32);
        Operations.FullExit memory x = Operations.FullExit({
            accountId:  0x010203,
            pubkey:     pubkey,
            owner:      0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            tokenId:    0x3132,
            nonce:      0x41424344,
            amount:     0x101112131415161718191a1b1c1d1e1f
        });

        bytes memory pubdata = Operations.writeFullExitPubdata(x);
        Operations.FullExit memory r = Operations.readFullExitPubdata(pubdata, 0);

        require(x.accountId == r.accountId, "accountId mismatch");
        require(x.owner == r.owner,         "owner mismatch");
        require(x.tokenId == r.tokenId,     "tokenId mismatch");
        require(x.nonce == r.nonce,         "nonce mismatch");
        require(x.amount == r.amount,       "amount mismatch");
        require(keccak256(x.pubkey) == keccak256(r.pubkey), "pubkey mismatch");
    }

    function testFullExitMatch(bytes calldata offchain) external pure returns (bool) {
        bytes memory pubkey = new bytes(32);
        Operations.FullExit memory x = Operations.FullExit({
            accountId:  0x010203,
            pubkey:     pubkey,
            owner:      0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5,
            tokenId:    0x3132,
            nonce:      0x41424344,
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

