// prefilled root for 1000 accs
// bytes32 constant EMPTY_TREE_ROOT = 0x09d809ed651bf1f19906bd7c170e1736176d3fbb2053e702dbbc2a8eed3e929f;

// empty root for 24
// bytes32 constant EMPTY_TREE_ROOT = 0x1d3843a9bbf376e57b3eca393198d7211882f6f2a76a53730243e2a1a519d92a;


// This contract is generated programmatically

pragma solidity ^0.4.24;


// Hardcoded constants to avoid accessing store
contract VerificationKeys {

    // For tree depth 24
    bytes32 constant EMPTY_TREE_ROOT = 0x09d809ed651bf1f19906bd7c170e1736176d3fbb2053e702dbbc2a8eed3e929f;

    function getVkUpdateCircuit() internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {

        
        vk[0] = 0x205689185631ff0d863a6f2da0d54da86eea23a7a8fc0b337f05dbdd022155e0;
        vk[1] = 0x23737e95ecc03889595f13d69d687a52eaf474cc7be053f0c5917c197fda6c57;
        vk[2] = 0x0580df889d6f7acb4dd7fc570e594693ed3b924ee1493ed08aa391145a2263f0;
        vk[3] = 0x23fa74c49362b2850d16b4caa4707fa23ca51cfc3642b6a83216276ead50a72b;
        vk[4] = 0x2279bb8162ff17c860ac0a44102fd93f04bda949daf9bba71d4a468669a529a2;
        vk[5] = 0x091cb662fd6e8330be410f91a0acb0e2527abca51ba828186c82df0841882677;
        vk[6] = 0x2b396d204de97ce0cfe7789b1082a4093ee99a6896f19fb897acdf71fa59d60e;
        vk[7] = 0x01f179302f8406d0996125f28327c1dce851c66feadbba2e996e8fd07daeef93;
        vk[8] = 0x09b4e2b7eb9afac2032548bb263fa6682e5dfe9ec87945cf9672a61d560fd8e4;
        vk[9] = 0x169b674db5165c17b0bea4dbae7aafe311d99663d3c632b42d60d210613b99b5;
        vk[10] = 0x0a1aa950839e7904d0c33ce72b23893402678dfe1aada5ad60178f2afe71fcdc;
        vk[11] = 0x04dea62d9770d36f621a6464cb084f41550703e533a9c9039fccacc779acc5f9;
        vk[12] = 0x2a0422ecea1055497501a7b980d7e06141769f227d8c3d1b6496bfabec371007;
        vk[13] = 0x1da6bd0a86cc0f3da9d3f1b0da18fddbf40d9122f28177a65114049f744f83da;

        gammaABC = new uint256[](8);
        gammaABC[0] = 0x0e4fc75aceed5ae90a59c80e09008930afcb2355bb5e924e56a6587e96b66b1f;
        gammaABC[1] = 0x272058762f1b6f03c8344ba4f8b83bdcf0d1f04ca6158c6ccb199716a586e01d;
        gammaABC[2] = 0x285cf2fe591f05a420b758264de59e4e2abe113f1ef91f224226844edff2014f;
        gammaABC[3] = 0x2c897ce90f715d81669847f76ce49ab9f76b3ae038efd17c79969fca04fbac6f;
        gammaABC[4] = 0x08030e083f89114e47beccbffa56bc128a18d8d1dab053bc985b7691d53f7340;
        gammaABC[5] = 0x01e3e96f8008b8de47cd4d7631dea90e1703273dc98ee91d7d47b49e18aa7aae;
        gammaABC[6] = 0x1f818ee7c3861156ee48a2f537cd324d77f9bde750cbccdb9433798ba8aa9af2;
        gammaABC[7] = 0x253e138c1535c273286f20e0a1b33159f017d0041f82fe84a8f80094821f55ba;


    }

}
