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
    bytes constant sha256MultiHash = hex"1220";
    bytes constant ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

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

    function tokenURI(uint256 tokenId) public view virtual override returns (string memory) {
        require(_exists(tokenId), "ne");
        string memory base = "ipfs://";
        string memory tokenContentHash = ipfsCID(_contentHashes[tokenId]);
        return string(abi.encodePacked(base, tokenContentHash));
    }

    /// @dev Converts hex string to base 58
    function toBase58(bytes memory source) internal pure returns (string memory) {
        uint8[] memory digits = new uint8[](46);
        digits[0] = 0;
        uint8 digitLength = 1;
        for (uint8 i = 0; i < source.length; ++i) {
            uint256 carry = uint8(source[i]);
            for (uint32 j = 0; j < digitLength; ++j) {
                carry += uint256(digits[j]) * 256;
                digits[j] = uint8(carry % 58);
                carry = carry / 58;
            }

            while (carry > 0) {
                digits[digitLength] = uint8(carry % 58);
                digitLength++;
                carry = carry / 58;
            }
        }
        return toAlphabet(reverse(digits));
    }

    function ipfsCID(bytes32 source) public pure returns (string memory) {
        return toBase58(abi.encodePacked(sha256MultiHash, source));
    }

    function reverse(uint8[] memory input) internal pure returns (uint8[] memory) {
        uint8[] memory output = new uint8[](input.length);
        for (uint8 i = 0; i < input.length; i++) {
            output[i] = input[input.length - 1 - i];
        }
        return output;
    }

    function toAlphabet(uint8[] memory indices) internal pure returns (string memory) {
        bytes memory output = new bytes(indices.length);
        for (uint32 i = 0; i < indices.length; i++) {
            output[i] = ALPHABET[indices[i]];
        }
        return string(output);
    }
}
