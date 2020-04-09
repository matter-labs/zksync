pragma solidity >=0.5.0 <0.7.0;

import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is VerifierWithDeserialize{
    constructor() public {}
    function initialize(bytes calldata) external {
    }

    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {
        if (_size == uint32(8)) { return true; }
        else { return false; }
    }

    function getVkBlock(uint32 _chunks) internal pure returns (VerificationKey memory vk) {
        if (_chunks == uint32(8)) { return getVkBlock8(); }
    }

    
    function getVkBlock8() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 2097152;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x032750f8f3c2493d0828c7285d0258e1bdcaa463f4442a52747b5c96639659bb);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x2ac1fae9c9951baecc95aef01be704d7842734b4829314e0e8066070baae653d,
            0x2e03458b2f0ca60f8028eb2344d24f5cc5dfaf95e7c4d1bda6fbea2dd7b8cd00
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x2ecacb87b85eea4b06e0897f959475ec927b948890da282c8c2890b8858de4b6,
            0x03f182dd7f23d1efc3af0ead2df56c2ffcee21c49289c9952da02d0e2e0560e9
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x09f0d594507118aad6b79b86b3deb5a68e3210a5f04c20a119412d8380ecd526,
            0x0486b3e1e70b2b1dab71658311976a32baf573f6857a2a03565313041f47ba5a
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x09ab8c8030099694ad48d794f24f64a2c4e1663482f7a420fe9bb51eb853d791,
            0x1ece97d6ea1b7ef0bf7a692c742eb1894a252459ace0930d5b213868ed7cf77e
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x219da0c1a0252c19192d8b829c791ce16a23ada0ed3c54ee709f44a290613154,
            0x1b0f10d4bc4c051548acd8c1411bd0bff86944d8ada7d307d5e13fbdff750d74
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x23e0e4f6ddc33d1a34ec8b71dc0efb09c79f49f395e999de084347154c17dc12,
            0x0b81f7f14f96df10fb252ffdbc44512f9097c6826a064f17a4e2e7153131fecb
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x2156ab1d6829b53eaa41458b326060e25c5e167ea6d13d1f3e7280c3d8ccdd93,
            0x2d16e2013d82c0cda3cc1b38ddb7c653696d67e8d57c550e5a3e89d0f4cf8d92
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x29948dd38debbbf236b3e8e39c259026febd9f3fe3e1b46fef0a2f894fa33f57,
            0x03ff29220a438b49b10439ff99d94d1574327b03cb5640b8cbca21583ecc32fd
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x21556abea64c63d00e1dfca27a5342387259b76cc30344c01daf0e094d5b1fe4,
            0x01c73dd91b82e5eeb60e0e2eb6d7740d14cf50b6f64ebe4566ea454c83b871d6
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x1b1ce22f890106cd07d38611f0c1584076622d0ba7fc7eb86dadf0a320fb39c5,
            0x2efc250c9f4c77a6a92ddb55e78f20ea3536478a343917537a4ca3891c35cb10
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x0463fe8926c360e033f7025616bf62348e638c04c86a281e96dfc940f2064c3d,
            0x1e78210069780cbdd8a08dc30a627f154dae5d641d837fbd0cb8d71102b22deb
        );

        vk.permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
             0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0],
            [0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
             0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55]
        );
    }
    
    function getVkExit() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 262144;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x0f60c8fe0414cb9379b2d39267945f6bd60d06a05216231b26a9fcf88ddbfebe);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x0cf301937441cdcf6ee11bb0e2c50514c92e4504f2ad9f00fb5b3ce8c9964ebb,
            0x08b5f877a8c47940059ed4e7dc8c5800f448219bfbfc06704b401ebc10ac74cc
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x2717152eabb71d05daae5ea915a84d022d67ef60065e741677aba36036271e33,
            0x015a7e474e507020b72f3dda8c2716ed04ec474c3f6288b58d941776a7096e2d
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x14907eccb82130fb54c243750132a3f6c92faf8c7e6e6e95fa03bd4a1b6134f9,
            0x2ee5ff73a32bde360dcec9dcb074ee5ffac1f2defcf18b7e460f12272dbc8f95
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x04a24a965bd99d9ab8aaccb13daf3a24e7ef052ef8a8b5c9dd35650b84751626,
            0x1503894d63e83c28c4141e91e4349b37e05ccb6ff1a3dbf5a50cc0eb9e208ad5
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x1b2932d07e953f6d2263ba3f8041f776ec59044ef116137c767d78996efdfd06,
            0x0b2e7ef84ab6946b3e29e3ffbec2373005935089a89f713124fbc249c38c4c3e
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x260edd8ae0bdc49e7cd7bf73b4483b55b1de35ab74a7fcf6941a0213ccdf11a4,
            0x23c889929746381beddd1fa457cf528100b6695585f0c254c30f58fae1dbc174
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x078a9b7a7e5cf8742a584265e8ec2ea055500d494f0bac571532300a77cd71ac,
            0x075199151e3e351e55357ed69c33fab4a0b502288ae19c139bce69e30ec8bb99
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x17881c6f5d00bdf70763fb7fbbbaafc788ee2048764850b5c5094f2e6d4d284f,
            0x1e8b62f7f3db45570dc07cf29787efed74ba22a55d8c9565a793a542ed65dc53
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x2d85767f3e785ee39f9081d3a0d78296dba3e930d04cd7e9c87ef549d27d8294,
            0x26914cd344f7e4913659466cba54124c6413b38ddcd75477ae8f58ec1e7f18f1
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x29b19b43c8e38d61b43a185bbc84e7415bc02e7e9eb4cbc5c8ae7bfae8525ca7,
            0x13f2daea3698c3c957e98ad83be967ac1af01fb5c3de4d03a87e45a685aa33a3
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x14516a29a4de7ba92459a9d29d4bd372359377741f61a10239b9b5f977e511ec,
            0x093ed9948b7e213518d8bc44a38d8b7f339aa8cea4a705391855bfd8d814632b
        );

        vk.permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
             0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0],
            [0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
             0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55]
        );
    }
    


    function verifyBlockProof(
        uint256[] calldata _proof,
        bytes32 _commitment,
        uint32 _chunks
    ) external view returns (bool) {
        uint256[] memory inputs = new uint256[](1);
        uint256 mask = (~uint256(0)) >> 3;
        inputs[0] = uint256(_commitment) & mask;
        Proof memory proof = deserialize_proof(1, inputs, _proof);
        VerificationKey memory vk = getVkBlock(_chunks);
        return verify(proof, vk);
    }

    function verifyExitProof(
        bytes32 _root_hash,
        address _owner,
        uint16 _tokenId,
        uint128 _amount,
        uint256[] calldata _proof
    ) external view returns (bool) {
        bytes32 commitment = sha256(abi.encodePacked(_root_hash, _owner, _tokenId, _amount));

        uint256[] memory inputs = new uint256[](1);
        uint256 mask = (~uint256(0)) >> 3;
        inputs[0] = uint256(commitment) & mask;
        Proof memory proof = deserialize_proof(1, inputs, _proof);
        VerificationKey memory vk = getVkExit();
        return verify(proof, vk);
    }
}
