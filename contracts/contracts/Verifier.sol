// from https://github.com/HarryR/ethsnarks/blob/master/contracts/Verifier.sol
pragma solidity 0.5.10;

import "./VerificationKey.sol";

contract Verifier is VerificationKey {
    bool constant DUMMY_VERIFIER = true;

    // Proof verification
    // Params:
    // - _proof - block number
    // - _commitment - block commitment
    function verifyBlockProof(
        uint256[8] calldata _proof,
        bytes32 _commitment
    ) external view returns (bool) {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(_commitment) & mask;
        return Verify(vk, gammaABC, _proof, inputs);
    }

    function verifyExitProof(
        uint16 _tokenId,
        address _owner,
        uint128 _amount,
        uint256[8] calldata _proof
    ) external view returns (bool) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_tokenId), uint256(_owner))
        );
        hash = sha256(abi.encodePacked(hash, uint256(_amount)));

        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(hash) & mask;
        return Verify(vk, gammaABC, _proof, inputs);
    }

    function NegateY(uint256 Y) internal pure returns (uint256) {
        uint256 q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        return q - (Y % q);
    }

    function Verify(
        uint256[14] memory in_vk,
        uint256[] memory vk_gammaABC,
        uint256[8] memory in_proof,
        uint256[] memory proof_inputs
    ) internal view returns (bool) {
        if (DUMMY_VERIFIER) {
            return true;
        }

        // Start
        require(
            ((vk_gammaABC.length / 2) - 1) == proof_inputs.length,
            "vvy11"
        ); // vvy11 - Invalid number of public inputs

        // Compute the linear combination vk_x
        uint256[3] memory mul_input;
        uint256[4] memory add_input;
        bool success;
        uint256 m = 2;

        // First two fields are used as the sum
        add_input[0] = vk_gammaABC[0];
        add_input[1] = vk_gammaABC[1];

        // Performs a sum of gammaABC[0] + sum[ gammaABC[i+1]^proof_inputs[i] ]
        for (uint256 i = 0; i < proof_inputs.length; i++) {
            mul_input[0] = vk_gammaABC[m++];
            mul_input[1] = vk_gammaABC[m++];
            mul_input[2] = proof_inputs[i];

            // solhint-disable-next-line no-inline-assembly
            assembly {
                // ECMUL, output to last 2 elements of `add_input`
                success := staticcall(
                    sub(gas, 2000),
                    7,
                    mul_input,
                    0x60,
                    add(add_input, 0x40),
                    0x40
                )
            }
            require(
                success,
                "vvy12"
            ); // vvy12 - Failed to call ECMUL precompile

            assembly {
                // ECADD
                success := staticcall(
                    sub(gas, 2000),
                    6,
                    add_input,
                    0x80,
                    add_input,
                    0x40
                )
            }
            require(
                success,
                "vvy13"
            ); // vvy13 - Failed to call ECADD precompile
        }

        uint256[24] memory input = [
            // (proof.A, proof.B)
            in_proof[0],
            in_proof[1], // proof.A   (G1)
            in_proof[2],
            in_proof[3],
            in_proof[4],
            in_proof[5], // proof.B   (G2)
            // (-vk.alpha, vk.beta)
            in_vk[0],
            NegateY(in_vk[1]), // -vk.alpha (G1)
            in_vk[2],
            in_vk[3],
            in_vk[4],
            in_vk[5], // vk.beta   (G2)
            // (-vk_x, vk.gamma)
            add_input[0],
            NegateY(add_input[1]), // -vk_x     (G1)
            in_vk[6],
            in_vk[7],
            in_vk[8],
            in_vk[9], // vk.gamma  (G2)
            // (-proof.C, vk.delta)
            in_proof[6],
            NegateY(in_proof[7]), // -proof.C  (G1)
            in_vk[10],
            in_vk[11],
            in_vk[12],
            in_vk[13] // vk.delta  (G2)
        ];

        uint256[1] memory out;
        assembly {
            success := staticcall(sub(gas, 2000), 8, input, 768, out, 0x20)
        }
        require(
            success,
            "vvy14"
        ); // vvy14 - Failed to call pairing precompile
        return out[0] == 1;
    }
}