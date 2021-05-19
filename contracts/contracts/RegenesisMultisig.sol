// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./Ownable.sol";
import "./Utils.sol";

pragma experimental ABIEncoderV2;

/// @title Regenesis Multisig contract
/// @author Matter Labs
contract RegenesisMultisig is Ownable {
    address[] public partners;

    uint32 public requiredNumberOfSignatures;
    uint32 public numberOfPartners;

    bytes32 public oldRootHash;
    bytes32 public newRootHash;

    constructor(address[] memory _partners, uint32 _requiredNumberOfSignatures) Ownable(msg.sender) {
        require(_requiredNumberOfSignatures <= _partners.length, "0");

        partners = _partners;
        numberOfPartners = uint32(_partners.length);
        requiredNumberOfSignatures = _requiredNumberOfSignatures;
    }

    function submitSignatures(
        bytes32 _oldRootHash,
        bytes32 _newRootHash,
        bytes[] memory _signatures
    ) external {
        requireMaster(msg.sender);

        bytes32 messageHash =
            keccak256(
                abi.encodePacked(
                    "\x19Ethereum Signed Message:\n157",
                    "OldRootHash:0x",
                    Bytes.bytesToHexASCIIBytes(abi.encodePacked(_oldRootHash)),
                    ",NewRootHash:0x",
                    Bytes.bytesToHexASCIIBytes(abi.encodePacked(_newRootHash))
                )
            );

        address[] memory recoveredAddresses = new address[](_signatures.length);
        for (uint32 i = 0; i < _signatures.length; i++) {
            recoveredAddresses[i] = Utils.recoverAddressFromEthSignature(_signatures[i], messageHash);
        }

        uint32 collectedSignatures = 0;
        for (uint32 i = 0; i < numberOfPartners; i++) {
            address partner = partners[i];

            for (uint256 signatureId = 0; signatureId < _signatures.length; signatureId++) {
                if (recoveredAddresses[signatureId] == partner) {
                    collectedSignatures += 1;
                    break;
                }
            }
        }

        require(collectedSignatures >= requiredNumberOfSignatures, "3"); // Not enough signatures

        oldRootHash = _oldRootHash;
        newRootHash = _newRootHash;
    }
}
