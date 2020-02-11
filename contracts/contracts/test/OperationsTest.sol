pragma solidity 0.5.16;

import "../Operations.sol";


contract OperationsTest {

    function testDeposit() external pure returns (bytes memory pubdata) {
        Operations.Deposit memory x = Operations.Deposit({
            tokenId: 77,
            amount: 123456789,
            owner: 0x823B747710C5bC9b8A47243f2c3d1805F1aA00c5
        });

        pubdata = Operations.writeDepositPubdata(x);
        Operations.Deposit memory r = Operations.readDepositPubdata(pubdata, 0);

        require(x.tokenId == r.tokenId, "tokenId mismatch");
        require(x.amount == r.amount,   "amount mismatch");
        require(x.owner == r.owner,     "owner mismatch");
    }
    
}

