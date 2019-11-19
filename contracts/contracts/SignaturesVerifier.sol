pragma solidity ^0.5.8;

import "./Bytes.sol";
import "./BlsOperations.sol";

library SignaturesVerifier {
    function verifyUserSignature(
        address _user,
        bytes memory _signature,
        bytes32 _messageHash
    ) internal pure returns (bool) {
        require(
            _signature.length == 65,
            "srve11"
        ); // srve11 - wrong user signature length
        uint8 v = uint8(_signature[0]);
        bytes memory rBytes = new bytes(32);
        for (uint8 i = 0; i < 32; ++i) {
            rBytes[i] = _signature[1 + i];
        }
        bytes32 r = Bytes.bytesToBytes32(rBytes);
        bytes memory sBytes = new bytes(32);
        for (uint8 i = 0; i < 32; ++i) {
            sBytes[i] = _signature[33 + i];
        }
        bytes32 s = Bytes.bytesToBytes32(sBytes);
        return ecrecover(_messageHash, v, r, s) == _user;
    }

    function verifyValidatorsSignature(
        BlsOperations.G2Point memory _aggrPubkey,
        bytes memory _signature,
        uint256 _messageHash
    ) internal view returns (bool) {
        require(
            _signature.length == 64,
            "srve21"
        ); // srve21 - wrong validators signature length
        bytes memory aggrSignatureXBytes = new bytes(32);
        for (uint8 i = 0; i < 32; ++i) {
            aggrSignatureXBytes[i] = _signature[i];
        }
        uint256 aggrSignatureX = Bytes.bytesToUInt256(aggrSignatureXBytes);

        bytes memory aggrSignatureYBytes = new bytes(32);
        for (uint8 i = 0; i < 32; ++i) {
            aggrSignatureYBytes[i] = _signature[32 + i];
        }
        uint256 aggrSignatureY = Bytes.bytesToUInt256(aggrSignatureYBytes);
        
        BlsOperations.G1Point memory mpoint = BlsOperations.messageHashToG1(_messageHash);
        BlsOperations.G1Point memory signature = BlsOperations.G1Point(aggrSignatureX, aggrSignatureY);
        return BlsOperations.pairing(mpoint, _aggrPubkey, signature, BlsOperations.negate(BlsOperations.generatorG2()));
    }
}