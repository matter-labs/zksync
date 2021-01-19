pragma solidity ^0.7.0;

contract TokenDeployInit {
    function getTokens() internal pure returns (address[] memory) {
        address[] memory tokens = new address[](4);
        tokens[0] = 0x948ba9Dc442B06e0ceD0DF437ff1bFE59433a74E;
        tokens[1] = 0x97C50a65AD238eF9C119368d3C802435e0f2433b;
        tokens[2] = 0xFcF74da8146bd16d43d3dA013440d986EcC8ED68;
        tokens[3] = 0x8E5a90cFEefd3F736E8E0FBf5EDbD5610d6e14Ca;
        return tokens;
    }
}
