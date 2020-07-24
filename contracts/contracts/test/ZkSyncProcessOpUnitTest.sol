pragma solidity ^0.5.0;

import "../generated/ZkSyncTest.sol";


contract ZkSyncProcessOpUnitTest is ZkSyncTest {

    function testProcessOperation(
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external {
        (bool blockProcessorCallSuccess, ) = blockProcessorAddress.delegatecall(
            abi.encodeWithSignature(
                "externalTestCollectOnchainOps(uint32,bytes,bytes,uint32[])",
                    uint32(0),
                    _publicData,
                    _ethWitness,
                    _ethWitnessSizes
            )
        );
        require(blockProcessorCallSuccess, "coo91"); // coo91 - `externalTestCollectOnchainOps` delegatecall fails
    }

}
