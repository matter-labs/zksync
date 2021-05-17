pragma solidity ^0.7.0;

import "./ZkSyncNFTFactory.sol";
import "./Governance.sol";

contract ZkSyncNFTCustomFactory is ZkSyncNFTFactory {
    Governance internal governance;

    constructor(
        string memory name,
        string memory symbol,
        address zkSyncAddress,
        address governanceZkSyncAddress
    ) ZkSyncNFTFactory(name, symbol, zkSyncAddress) {
        governance = Governance(governanceZkSyncAddress);
    }

    function registerNFTFactory(
        uint32 _creatorAccountId,
        address _creatorAddress,
        bytes memory _signature
    ) external {
        governance.registerNFTFactoryCreator(_creatorAccountId, _creatorAddress, _signature);
    }
}
