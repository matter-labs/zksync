// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./NFTFactory.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract ZkSyncNFTFactory is ERC721, NFTFactory {
    // Optional mapping for token content hashes
    mapping(uint256 => bytes32) private _contentHashes;
    address private _zkSyncAddress;

    constructor(
        string memory name,
        string memory symbol,
        address zkSyncAddress
    ) ERC721(name, symbol) {
        _zkSyncAddress = zkSyncAddress;
    }

    function mintNFTFromZkSync(
        address creator,
        address recipient,
        uint32 serialId,
        bytes32 contentHash,
        uint32 tokenId
    ) external override {
        require(_msgSender() == _zkSyncAddress, "z"); // Minting allowed only from zkSync
        _safeMint(recipient, tokenId);
        _contentHashes[tokenId] = contentHash;

        emit MintNFTFromZkSync(creator, recipient, serialId, contentHash, tokenId);
    }

    function _beforeTokenTransfer(
        address,
        address to,
        uint256 tokenId
    ) internal virtual override {
        // Sending to address `0` means that the token is getting burned.
        if (to == address(0)) {
            delete _contentHashes[tokenId];
        }
    }
}
