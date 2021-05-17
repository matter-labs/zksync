// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSyncNFTFactory.sol";

contract ZkSyncNFTFactoryUnitTest is ZkSyncNFTFactory {
    constructor(
        string memory name,
        string memory symbol,
        address zkSyncAddress
    ) ZkSyncNFTFactory(name, symbol, zkSyncAddress) {}

    function getBitsPublic(
        uint256 number,
        uint16 bitFrom,
        uint16 bitTo
    ) public pure returns (uint256) {
        return getBits(number, bitFrom, bitTo);
    }
}
