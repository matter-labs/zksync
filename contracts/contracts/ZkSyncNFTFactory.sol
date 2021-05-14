// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./NFTFactory.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract ZkSyncNFTFactory is ERC721, NFTFactory {
    /// @notice Packs address and token id into single word to use as a key in balances mapping
    function packCreatorFingerprint(
        address creatorAddress,
        uint32 creatorId,
        uint32 serialId
    ) internal pure returns (uint256) {
        return uint256(creatorAddress) | (uint256(creatorId) << 160) | (uint256(serialId) << 192);
    }

    // Optional mapping for token content hashes
    mapping(uint256 => bytes32) private _contentHashes;

    // Optional mapping for creator fingerprints. Each looks as concat(creatorAddress | creatorId | serialId)
    mapping(uint256 => uint256) private _creatorFingerprints;

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
        uint32 creatorAccountId,
        address recipient,
        uint32 serialId,
        bytes32 contentHash,
        uint32 tokenId
    ) external override {
        require(_msgSender() == _zkSyncAddress, "z"); // Minting allowed only from zkSync
        _safeMint(recipient, tokenId);
        _contentHashes[tokenId] = contentHash;
        uint256 creatorFingerprint = packCreatorFingerprint(creator, creatorAccountId, serialId);
        _creatorFingerprints[tokenId] = creatorFingerprint;

        emit MintNFTFromZkSync(creator, creatorAccountId, recipient, serialId, contentHash, tokenId);
    }

    function _beforeTokenTransfer(
        address,
        address to,
        uint256 tokenId
    ) internal virtual override {
        // Sending to address `0` means that the token is getting burned.
        if (to == address(0)) {
            delete _contentHashes[tokenId];
            delete _creatorFingerprints[tokenId];
        }
    }

    function getContentHash(uint256 _tokenId) external view returns (bytes32) {
        return _contentHashes[_tokenId];
    }

    function getCreatorFingerprint(uint256 _tokenId) external view returns (uint256) {
        return _creatorFingerprints[_tokenId];
    }

    // Retrieves the bits from firstOne to lastOne bits
    function getBits(
        uint256 number,
        uint8 firstOne,
        uint8 lastOne
    ) internal pure returns (uint256) {
        require(lastOne > firstOne, "qq");

        // So here we are creating a mask which consists of only ones
        // from the firstOne bit to the lastOne bit
        uint256 a = (1 << lastOne) - 1;
        uint256 b = (1 << firstOne) - 1;
        uint256 mask = a ^ b;

        uint256 onlyNeededBits = (number & mask);
        return onlyNeededBits >> firstOne;
    }

    function getCreatorAddress(uint256 tokenId) external view returns (address) {
        uint256 fingerPrint = uint256(_creatorFingerprints[tokenId]);

        return address(getBits(fingerPrint, 0, 160));
    }

    function getCreatorAccountId(uint256 tokenId) external view returns (uint32) {
        uint256 fingerPrint = uint256(_creatorFingerprints[tokenId]);

        return uint32(getBits(fingerPrint, 160, 192));
    }

    function getSerialId(uint256 tokenId) external view returns (uint32) {
        uint256 fingerPrint = uint256(_creatorFingerprints[tokenId]);

        return uint32(getBits(fingerPrint, 192, 224));
    }
}
