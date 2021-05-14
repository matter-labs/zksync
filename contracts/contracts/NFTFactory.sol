// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

interface NFTFactory {
    function mintNFTFromZkSync(
        address _creator,
        uint32 _creatorAccountId,
        address _recipient,
        uint32 _serialId,
        bytes32 _contentHash,
        uint32 _tokenId
    ) external;

    event MintNFTFromZkSync(
        address indexed creator,
        uint32 indexed creatorAccountId,
        address indexed recipient,
        uint32 serialId,
        bytes32 contentHash,
        uint32 tokenId
    );
}
