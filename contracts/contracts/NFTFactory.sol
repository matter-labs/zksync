// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

interface NFTFactory {
    function mintNFTFromZkSync(
        address creator,
        address recipient,
        uint32 creatorAccountId,
        uint32 serialId,
        bytes32 contentHash,
        // Even though the token id can fit into the uint32, we still use
        // the uint256 to preserve consistency with the ERC721 parent contract
        uint256 tokenId
    ) external;

    event MintNFTFromZkSync(
        address indexed creator,
        address indexed recipient,
        uint32 creatorAccountId,
        uint32 serialId,
        bytes32 contentHash,
        uint256 tokenId
    );
}
