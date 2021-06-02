// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./NFTFactory.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";

contract ZkSyncNFTFactory is ERC721, NFTFactory {
    uint8 constant ADDRESS_FOOTPRINT_OFFSET = 0;
    uint8 constant ADDRESS_SIZE_BITS = 160;

    uint8 constant CREATOR_ID_FOOTPRINT_OFFSET = ADDRESS_FOOTPRINT_OFFSET + ADDRESS_SIZE_BITS;
    uint8 constant CREATOR_ID_SIZE_BITS = 32;

    uint8 constant SERIAL_ID_FOOTPRINT_OFFSET = CREATOR_ID_FOOTPRINT_OFFSET + CREATOR_ID_SIZE_BITS;
    uint8 constant SERIAL_ID_SIZE_BITS = 32;

    /// @notice Packs address and token ID into single word to use as a key in balances mapping
    function packCreatorFingerprint(
        address creatorAddress,
        uint32 creatorId,
        uint32 serialId
    ) internal pure returns (uint256) {
        return (// shift address by zero bits to preserve consistency
        (uint256(creatorAddress) << ADDRESS_FOOTPRINT_OFFSET) |
            (uint256(creatorId) << CREATOR_ID_FOOTPRINT_OFFSET) |
            (uint256(serialId) << SERIAL_ID_FOOTPRINT_OFFSET));
    }

    // Optional mapping from token ID to token content hash
    mapping(uint256 => bytes32) private _contentHashes;

    // Optional mapping from token ID to creator fingerprints -- concat(creatorAddress | creatorId | serialId)
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
        address recipient,
        uint32 creatorAccountId,
        uint32 serialId,
        bytes32 contentHash,
        uint256 tokenId
    ) external override {
        require(_msgSender() == _zkSyncAddress, "z"); // Minting allowed only from zkSync
        _safeMint(recipient, tokenId);
        _contentHashes[tokenId] = contentHash;
        uint256 creatorFingerprint = packCreatorFingerprint(creator, creatorAccountId, serialId);
        _creatorFingerprints[tokenId] = creatorFingerprint;

        emit MintNFTFromZkSync(creator, recipient, creatorAccountId, serialId, contentHash, tokenId);
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

    // Retrieves the bits from firstOne to lastOne bits. The range is exclusive.
    // This means that if you want to get bits from the zero-th to the first one, then
    // bitFrom = 0, bitTo = 2
    function getBits(
        uint256 number,
        uint16 bitFrom,
        uint16 bitTo
    ) internal pure returns (uint256) {
        require(bitTo > bitFrom, "qq");

        // So here we are creating a mask which consists of only ones
        // from the firstOne bit to the lastOne bit
        uint256 a = (1 << bitFrom) - 1;
        uint256 b = (1 << bitTo) - 1;
        uint256 mask = a ^ b;

        uint256 onlyNeededBits = (number & mask);
        return onlyNeededBits >> bitFrom;
    }

    function getCreatorAddress(uint256 tokenId) external view returns (address) {
        uint256 fingerPrint = _creatorFingerprints[tokenId];

        return address(getBits(fingerPrint, ADDRESS_FOOTPRINT_OFFSET, ADDRESS_FOOTPRINT_OFFSET + ADDRESS_SIZE_BITS));
    }

    function getCreatorAccountId(uint256 tokenId) external view returns (uint32) {
        uint256 fingerPrint = _creatorFingerprints[tokenId];

        return
            uint32(
                getBits(fingerPrint, CREATOR_ID_FOOTPRINT_OFFSET, CREATOR_ID_FOOTPRINT_OFFSET + CREATOR_ID_SIZE_BITS)
            );
    }

    function getSerialId(uint256 tokenId) external view returns (uint32) {
        uint256 fingerPrint = _creatorFingerprints[tokenId];

        return
            uint32(getBits(fingerPrint, SERIAL_ID_FOOTPRINT_OFFSET, SERIAL_ID_FOOTPRINT_OFFSET + SERIAL_ID_SIZE_BITS));
    }
}
