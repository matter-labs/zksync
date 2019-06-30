// from https://github.com/HarryR/ethsnarks/blob/master/contracts/Verifier.sol
pragma solidity ^0.5.8;

contract Verifier {

    function NegateY( uint256 Y )
        internal pure returns (uint256)
    {
        uint q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        return q - (Y % q);
    }

    function Verify (uint256[14] memory in_vk, uint256[] memory vk_gammaABC, uint256[8] memory in_proof, uint256[] memory proof_inputs)
        internal
        view
        returns (bool)
    {
        require(((vk_gammaABC.length / 2) - 1) == proof_inputs.length, "Invalid number of public inputs");

        // Compute the linear combination vk_x
        uint256[3] memory mul_input;
        uint256[4] memory add_input;
        bool success;
        uint m = 2;

        // First two fields are used as the sum
        add_input[0] = vk_gammaABC[0];
        add_input[1] = vk_gammaABC[1];

        // Performs a sum of gammaABC[0] + sum[ gammaABC[i+1]^proof_inputs[i] ]
        for (uint i = 0; i < proof_inputs.length; i++) {
            mul_input[0] = vk_gammaABC[m++];
            mul_input[1] = vk_gammaABC[m++];
            mul_input[2] = proof_inputs[i];

            // solhint-disable-next-line no-inline-assembly
            assembly{
                // ECMUL, output to last 2 elements of `add_input`
                success := staticcall(sub(gas, 2000), 7, mul_input, 0x60, add(add_input, 0x40), 0x40)
            }
            require(success, "Failed to call ECMUL precompile");

            assembly{
                // ECADD
                success := staticcall(sub(gas, 2000), 6, add_input, 0x80, add_input, 0x40)
            }
            require(success, "Failed to call ECADD precompile");
        }

        uint[24] memory input = [
            // (proof.A, proof.B)
            in_proof[0], in_proof[1],                           // proof.A   (G1)
            in_proof[2], in_proof[3], in_proof[4], in_proof[5], // proof.B   (G2)

            // (-vk.alpha, vk.beta)
            in_vk[0], NegateY(in_vk[1]),                        // -vk.alpha (G1)
            in_vk[2], in_vk[3], in_vk[4], in_vk[5],             // vk.beta   (G2)

            // (-vk_x, vk.gamma)
            add_input[0], NegateY(add_input[1]),                // -vk_x     (G1)
            in_vk[6], in_vk[7], in_vk[8], in_vk[9],             // vk.gamma  (G2)

            // (-proof.C, vk.delta)
            in_proof[6], NegateY(in_proof[7]),                  // -proof.C  (G1)
            in_vk[10], in_vk[11], in_vk[12], in_vk[13]          // vk.delta  (G2)
        ];

        uint[1] memory out;
        assembly {
            success := staticcall(sub(gas, 2000), 8, input, 768, out, 0x20)
        }
        require(success, "Failed to call pairing precompile");
        return out[0] == 1;
    }
}