// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

interface NFTFactory {
    function mintNFT(
        address creator,
        address recipient,
        uint32 serialId,
        bytes32 contentHash,
        uint32 tokenId
    ) external;

    event MintNFT(
        address indexed creator,
        address indexed recipient,
        uint32 serialId,
        bytes32 contentHash,
        uint256 tokenId
    );
}
