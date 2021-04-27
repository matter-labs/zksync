pragma solidity ^0.7.0;

import "./NFTFactory.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";

/**
 * @title ZkSync Default ERC721 Non-Fungible Token Standard basic implementation
 * @dev see https://eips.ethereum.org/EIPS/eip-721
 */
contract ZkSyncNFT is ERC721, NFTFactory {

    // Optional mapping for token content hashes
    mapping(uint256 => bytes32) private _contentHashes;
    address private _zksync_address;

    constructor(
        string memory name,
        string memory symbol,
        address zksync_address
    ) ERC721(name, symbol) {
        _zksync_address = zksync_address;
    }

    function mintNFT(
        address _creator,
        address recipient,
        bytes32 contentHash,
        uint256 tokenId
    ) external override {
        require(_msgSender() == _zksync_address, "Miniting allowed only from zksync");
        _safeMint(recipient, tokenId);
        _contentHashes[tokenId] = contentHash;
    }

    function _beforeTokenTransfer(address from, address to, uint256 tokenId) override internal virtual {
        // That means token is burning
        if (to == address(0)) {
            delete _contentHashes[tokenId];
        }
    }
}
