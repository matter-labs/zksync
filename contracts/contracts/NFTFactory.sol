// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

interface NFTFactory {
    function mintNFT(
        address creator,
        address recipient,
        bytes32 contentHash,
        uint256 tokenId
    ) external;

    event MintNFT(address indexed creator, address indexed recipient, bytes contentHash, uint256 tokenId);
}