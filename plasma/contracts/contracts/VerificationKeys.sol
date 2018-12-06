// prefilled root
// bytes32 constant EMPTY_TREE_ROOT = 0x0a4a8a376264cf0603af6916e662020a4d73ec5eb538e9879e2546c7d64ec289;

// empty root for 24
// bytes32 constant EMPTY_TREE_ROOT = 0x1d3843a9bbf376e57b3eca393198d7211882f6f2a76a53730243e2a1a519d92a;

// This contract is generated programmatically

pragma solidity ^0.4.24;


// Hardcoded constants to avoid accessing store
contract VerificationKeys {

    // For tree depth 24
    bytes32 constant EMPTY_TREE_ROOT = 0x0a4a8a376264cf0603af6916e662020a4d73ec5eb538e9879e2546c7d64ec289;

    function getVkUpdateCircuit() internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {

        
        vk[0] = 0x0821db8fd226d32f376069634e5fae7584404751c54a2dc078d21815cd29ebdc;
        vk[1] = 0x24df5675fb25b1d3ebe887a6a9fb9a5e9333405f0946eba634ffeff7f1a3b046;
        vk[2] = 0x179f489f59ee02e5d181fcfae7477aed918c6ff464d5b27bf67afeca1c907d34;
        vk[3] = 0x27f998f4e12dad86fb293917dc868cadcbbc2e92e4e14f55aa4f9957bc0acaff;
        vk[4] = 0x1ba333df83d312f57059afc19bb90248c29f27198e9987cec0fa3ef0c646892c;
        vk[5] = 0x21433e2731aa26820ad40aafb000ba5d6edfd9411a0308765cc6716d018f0750;
        vk[6] = 0x0baaf0e2a8b730ac65ff246865b7e95c70ab3a3300b48f6647d81ccf8c07bf3b;
        vk[7] = 0x270e5254676dfc8228ea52f14ae31f08529cb671dc25f679f54b361da05f7e6e;
        vk[8] = 0x0a2a651ae4a401fc1d1b869c580a149cc799909a8e0e91f809ead8e5fecb40d6;
        vk[9] = 0x09ccbedfa42a77886d4aaf05b93b53ca993c2b2fd74d384f701ff6a36336c82a;
        vk[10] = 0x0801eb631bfaf0f787b056f8a2e60963a3d6f233867a621e17dad42ddc341888;
        vk[11] = 0x24693a927e6e1fe5537b09daf38bf5c50363e433c2cbdde62f39b0ad1c81f975;
        vk[12] = 0x12a3f2caf69178478bbce88def317ca2d5c3d33c1c92b5ea8773649f3ef9a4a1;
        vk[13] = 0x1089cb7f92d00c54fada2741606dad66a97dffc009fb471d7cd80fecc7e8aaff;

        gammaABC = new uint256[](8);
        gammaABC[0] = 0x0c89c064b964706f01748636b1558d97b09fc305c192ad2ca014497878cd6603;
        gammaABC[1] = 0x12c3568a1a1c2ba24202e99926017455406ce28e0327f83ccd284151b52cf93e;
        gammaABC[2] = 0x0c5a08c8d16c4d87fbc2c92a25b9d249baa2fa4cb5bbbd3dffca19d11109abd6;
        gammaABC[3] = 0x1e7224763f1fbf54e9decefeff303bf565cac1f3b16237ef7412234ef4284bb8;
        gammaABC[4] = 0x213f0c597b57cb4b2a4cc2b7c49946125c8817a51686d0ab12fa46d807df7943;
        gammaABC[5] = 0x2d9c9b677740042182c6bde86f55745cd2b607753ac808ad42f51f73e5045747;
        gammaABC[6] = 0x1d70f763fdfb0a5b555e9794030dacb43d6cc3b8d7ec5aeb3acfa81cd460cfb2;
        gammaABC[7] = 0x20fd53f5d95aba2349f63cbdb47ede913af2e5d36f1a50f162c280b6d131e72c;


    }

}
