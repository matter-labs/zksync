// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

interface MintNFT {

    function mintNFT(address creator, address recipient, bytes contentHash) external view returns (bool);

    event MintNFT(address indexed creator,address indexed recipient, bytes contentHash);
}
