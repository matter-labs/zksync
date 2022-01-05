// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

contract DummyUpgradeGatekeeper {
    address[] public nextTargets;

    function setNextTargets(address[] calldata _targets) external {
        nextTargets = _targets;
    }
}
