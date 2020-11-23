pragma solidity >=0.5.0 <0.8.0;

// SPDX-License-Identifier: MIT OR Apache-2.0



contract TokenDeployInit {
    function getTokens() internal pure returns (address[] memory) {
        address[] memory tokens = new address[](4);
        tokens[0] = 0x597bC079515395c69c91Dc4c14C459D343cba416;
        tokens[1] = 0xd096C1a60b9010Fa5C8b3d3d1578C9701BdA7274;
        tokens[2] = 0xF756a8fC79bfF7b4698e14dd03B91ea1840A0Fd7;
        tokens[3] = 0x45A24Be6399B0C39cbf5B5e3981d2C050bF17336;
        return tokens;
    }
}
