// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./NFTFactory.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract ZkSyncNFTFactory is ERC721, NFTFactory {
    // Optional mapping for token content hashes
    mapping(uint256 => bytes32) private _contentHashes;
    address private _zksyncAddress;

    constructor(
        string memory name,
        string memory symbol,
        address zksyncAddress
    ) ERC721(name, symbol) {
        _zksyncAddress = zksyncAddress;
    }

    function mintNFT(
        address,
        address recipient,
        bytes32 contentHash,
        uint32 tokenId
    ) external override {
        require(_msgSender() == _zksyncAddress, "z"); // Minting allowed only from zksync
        _safeMint(recipient, tokenId);
        _contentHashes[tokenId] = contentHash;
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
