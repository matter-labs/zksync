pragma solidity ^0.7.0;

import "./../Governance.sol";

contract TestGovernance is Governance {
    function publicPackRegisterNFTFactoryMsg(
        uint32 _creatorAccountId,
        address _creatorAddress,
        address _factoryAddress
    ) external view returns (bytes memory) {
        return packRegisterNFTFactoryMsg(_creatorAccountId, _creatorAddress, _factoryAddress);
    }
}
