pragma solidity >=0.5.0 <0.7.0;

import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is VerifierWithDeserialize{

    bool constant DUMMY_VERIFIER = false;

    constructor() public {}
    function initialize(bytes calldata) external {
    }

    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {
        if (_size == uint32(8)) { return true; }
        else if (_size == uint32(32)) { return true; }
        else if (_size == uint32(76)) { return true; }
        else if (_size == uint32(168)) { return true; }
        else if (_size == uint32(352)) { return true; }
        else if (_size == uint32(718)) { return true; }
        else { return false; }
    }

    function getVkBlock(uint32 _chunks) internal pure returns (VerificationKey memory vk) {
        if (_chunks == uint32(8)) { return getVkBlock8(); }
        else if (_chunks == uint32(32)) { return getVkBlock32(); }
        else if (_chunks == uint32(76)) { return getVkBlock76(); }
        else if (_chunks == uint32(168)) { return getVkBlock168(); }
        else if (_chunks == uint32(352)) { return getVkBlock352(); }
        else if (_chunks == uint32(718)) { return getVkBlock718(); }
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
    
    function getVkBlock32() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 4194304;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x18c95f1ae6514e11a1b30fd7923947c5ffcec5347f16e91b4dd654168326bede);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x27e0fbdeedb91b323d241ee51a449981b5d418967daeb1d4b1f3c7d681c52208,
            0x1562dfb8c8f52c80d7ca72079278b39a9c6db6d1fbedaf1cca1e95fc3f540d8a
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x185de07ddbb6060cdefd02fb45cf7547545488329042578fff4c8cd1ade23fd1,
            0x1ab90f419f64edd044baf4457c76e6e45785787369d20f2ff69a6634eac499e0
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x09841e4845f579c25207fb0156e86c6ce52edba38eb3d164fcb2f975ff5e36e0,
            0x1b76fd46e375ed74e8e4eedea2c02b067fd2716f5b3064d684399370f1a1653c
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x1ef6e25c8b690c63a94c77c56b2a8f2dfcf7fe2acd28c56dd684f1252d8bd080,
            0x1adb3aaae60ae01878a62914a691c5fecbf03a822e0b1647625e955ae0e7136a
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x0a599a6a6e117cdaf9733fdfc5ab616f3405b1a6576910be184bdbfdc855831a,
            0x1d969f41879e207d459a2030a4449e61d3a901aa5b296dce5ec4a4df3d6db5a7
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x165a96730c02fded7763a34161757a4173a0c6de2152321be4a522e21ac706fd,
            0x0852e7bd1504311d4cb988d6a2ce0d6a37a1a883ad4303b9517b82a2124012b5
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x240d0e4b8cdc3173fb1c04fc881a679887d2a71921bf6f91a9de2bd0c27c5f6e,
            0x296be53f238386dc525d4808feb3d4e5d7b8f28dabeff7963270235f892ff38e
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x2f2d526498e234bb779c4edf7e43eeb3dd203682fe48503147412491910f500d,
            0x2a57681e32c496b8adf73c8478267883b14b63c48bb1a96eaf4a3367e0f610bf
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x00ddd0deac6646d2d1349cc0269eeae2d27fde7cdb16c6664d1282d2ac6cd6ac,
            0x1029b11c19f069fe359ef8e28e0018471f5767d0e33bea1f9859012852c614bb
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x0ee623215e8c4709d2b8f2992bc730d19c8902b3e46ace0f4bcb1ec70da0e312,
            0x0d0c01dab87978962ac5f975afe3bc2f54d0ba290503075f7eec1a4053a25c6d
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x1f16203e5e2a5f774603bac724e74bd544d7e528a26e67493254d575109ec0bc,
            0x2973066ff04f35480f784957ab729531ee7c5ee26fd48b4bf0e1037f09d020fe
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
    
    function getVkBlock76() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 8388608;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1283ba6f4b7b1a76ba2008fe823128bea4adb9269cbfd7c41c223be65bc60863);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x1981dee2189deaf1a195593504f2778c244c87bb05ed4cc2d6cdcb10994c8ee0,
            0x0ff8c2464a465231548e39926cf031e579790a80af103797c576a2a851131106
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x03d35ff69f7f3c63b7493b8c487bcfcaa6cc535014b269ca72484103fa84908c,
            0x13cd66a0704aa34b4e10a844ad1a401d34b3d2b803c14d44d8368c86da659782
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x112b7433ccc54b87bab746c7f01560819d1fed8a7689fa9b0680f0385cd39c18,
            0x22f21a2a790a189bc7e5acf2f47c2db9061a4ed9e5e342e7200d369d9c07bc7f
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x0ecd6c12d5de1344509a7246832147126e3e5c31b4216c82447a20711610ad06,
            0x246274b4d691ee6a1a537d4caa38783e70d38094eb6bf75d00abf9381037e23c
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x1fac583560e396be4fe24d161d5cd74882a2afe0f9f8c0949f760ccb348ea360,
            0x25f307c76c13ee17d938b4f8871868284922bbcad738ac297964e707ac50fbc0
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x1b9c8e75cd45728a5578bbde951f79709192d5155f9caa1d2916a21bb2e329ab,
            0x2645ec7b7aeb2e1077568a8c735f3eaf874134db0e7fcbc76e61f6973edeeb8b
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x1fe74c428e150925429cd34076c4276e712e8446885e169257cc63cef53de7b0,
            0x16f9908522594c1b82a8a9eaf5bcc0476132ebe389dc15461a50cc50cc60f3be
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x0bd51129ab50c99b522431be6a4a8140697b84be38045a215e91755956105524,
            0x0ce538acdaa48f8a4b94745559891c77d64cc36327c452b15e37d1ea0a20eb8a
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x14edaf0ec11c2ac29f589b716bae2b2cf7d3d935c2d753d24cd8125a71336ad1,
            0x0ccd5f41e826fe11cd24bdb338d1ada7c6e3a50c120e9fa75c7402b89d2df975
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x2db35c7d64f5676a16cb3ce1d6688ab068188727b7d61255832ccb3dbb3a5862,
            0x2672fd13902fde16df2f800fd951e18c292b400190116be6f1243e0b0090f62d
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x13240960eb219ce17fa368a8159f0ac430895dc6c0decd2d70dd4da7d2a3dfe6,
            0x0c395fbe7ad12fff520f11b3b88877fbf4fcdb18bce999d11f6c0b1e26e002f6
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
    
    function getVkBlock168() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 16777216;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1951441010b2b95a6e47a6075066a50a036f5ba978c050f2821df86636c0facb);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x097505a0ad0109935b4fc2f9ca9288636bff08d43cfc6a6dea73961e28d81d25,
            0x16cbea8aecc0fe42803b5e081acec24a9c8d93e3bfd5edcde0204c24e07f4b73
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x15655a5269af48c0effb7d7a88857ef0c1541f236d3ee685b48d6860612f3490,
            0x20e495070badc19682003578ca2d7a28b7b22b5cc8a063237accefcdd2d1c714
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x0409f0b7d086f4b117ef7bd80e9503fad0a4be78c58e5506705896375c0a1f13,
            0x1933f7bd271f8c08ddd5f78ac19be480146d829aaa4b737d05b98a03b73babce
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x1cecbb9425b84bad8d5c9d1a06b92f75b084ee15f5d5e47c112012a5bf732689,
            0x2869a533ce7c9f033450506f892dfcca907abf769b700aae80ebc49e6fa7e1d1
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x064e309bed34d6eef1233f645baa8796ecc25712dd7d9022c94b42e7b4675071,
            0x1c8534dce6acdedc333c94f58996f12ccc4bbb424c852b406682589e1e5585f0
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x18b5a8ba90ffa3c17c8b60fd98899d6f6cdfdb4f0fc6f3604f8c911289fcf166,
            0x108e0fb07387df22919071c8ae66fd0af066edb4a76a4f24f2a3980dc4a3eca3
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x24b636f92bb6a4f07fc7251615713730e25e976a84f9f16f1480ba064d008d68,
            0x1ffdc50e4643a8cbb807f5dcc72dd100ce5781a54e581915cc78a2d6b5482477
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x021cf86f9b03726a6795e94aa6a2385582261d1988dac6d15e330986cfa76e30,
            0x0e5d459278000369462cbfbd3f57a82708848b66efa7cb0c733ce0bbe5502148
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x02abf8af4d3e70cee6a24711c82ec5fbb0a8cda0a30f19ff34e57f4158829bef,
            0x0fe840632f78ac917b694782b843b4fc4181d6e7c32b8e271f4bf0039b32ac30
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x0f0bd6ee80f400314ab6f7f9f0865ba98aa04c28ca71a5608244c674ad281eb0,
            0x26859488627bc76b0937d0791f9e8b5e0c58e9a75873dcf8e6ba7894f277dd07
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x2717fd02c770795c833edf2ebade395c6f252390c5d9a7c91e676f2aff042710,
            0x2131c169d33cb34342961afb4d4c1ae4109fde40e01142d250a7ba03508bf965
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
    
    function getVkBlock352() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 33554432;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x0d94d63997367c97a8ed16c17adaae39262b9af83acb9e003f94c217303dd160);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x15779314f324967a10915b9fd2faf64f083ea4865399bb2cb36b862d73c0e2e7,
            0x14ec2d76785112b41551efe06dbcb8ab51fc33a36c822aec64e676c93e1e8674
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x123573934927ef12b186873b1d68cd94a300de4a89d778fef720b15a64d91c2a,
            0x141dd981742c9639ec2aeabfc0dfe2a77256566004906f2731b969e146e906be
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x26c0daebf9d942a7fb185840f3b799640e8bdd629e8e83c30d57d82ff761359b,
            0x1ad948237fff4807d32d80d2fd80615589a14ef3ce34dfbf53bbe17d0ff7f607
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x059ebc536efd50fcbcfcb74a76f10bce2a930a69b59e05ba22b4fb057f1c444f,
            0x306404b5bcb3ed566664ad817cbdb757b3cbafc63a089d598376ddc2effeab63
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x002d6cce4553cd0a65fe180f8d24f5a976efb5bb1588dd98d3ed5fac5c3aedba,
            0x0ca11b59b69c9f5a32dcbe62b9c5d6c6e2db7f5bc9f75927b987063fb440afd6
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x2195b7ab01b6c3cd3ff6f58dcc551ab212733db6f4023fa279ce7f29c1bb9086,
            0x18cfbade956ca8e8d94e53c97811b76a6ce7811b329fdf1112bad867fb8d22ad
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x232d97f1350ea55363a566e4791fe8a5c8e80fd36bf8b74a93e7a241dc4a1ba6,
            0x0f3fa0519fd23b27ba7015f55d0a99c055818eba89e8e059c9caa898550f0778
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x15d2c09209244c7ebda7806afe7b472777d9d2d3df16c0d8dbe64623828bd78e,
            0x18b3fa40e9dc43346b1f82a5716a2e684ecb885983218a443af9c953bdab9ec8
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x27b1700b5dfb5f03b7801a0864a4ae89e1cbae0036cae25fbc032932f5d849f5,
            0x301de00d6161fc5c705482e9430827352b0bf23328401ad275ed2b2ade48e334
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x21159356573c6afba5b720fbb12968b145c9ea7734eda0ec55f357484a9b08e4,
            0x17251c49daaf7f98624dd98f336c81028f9fd449a4070c6a1e1f725594b3ce78
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x2e51d9b2e78a2cb16ca84fdea0d96cea6afa8a58f04e02e3719849d35854ec3f,
            0x2c0acf8a7580494360806c7016cfd852adc01ced19cd55f012bb591a64143c6d
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
    
    function getVkBlock718() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 67108864;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1dba8b5bdd64ef6ce29a9039aca3c0e524395c43b9227b96c75090cc6cc7ec97);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x23c670a5a3e08f9344a9e593281ad2a6f6f99d5cc69c89684da218355c83680f,
            0x14a64f11fc20561e7a17f8d95b6896adb92ad6eace3d19e5a4dae903a3261d06
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x2e51baeafcea6f56231138db1ffc54bd2e5b5e6dd5773faf6bb3a0200bb1dac1,
            0x02326782c4573322aa438068a3bd93c0eabeb04cad515f024f62632d66dfa113
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x0a2fd87219c1bd5dfcbdf955c139ac4718d3392bfeabdedd0a24e41eea386123,
            0x15adaa06edc4c713068398bf9c16b600828df9bbb15d279b6c164aafcdc1f158
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x0054a47218d2cf5978d2e8d01ba7a8f2b84378ee02ad6321c736aaee436c9f73,
            0x020b4565cfaaf44678867a5c8cdf548ad4038afd29f260f06a490549c88ef836
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x1e45ca852ca0f89a50102c5e24b48cba92a9cbe51ebda1d6020c871d99632b82,
            0x25a2abc1b6d92786319773643eb62939e2a329dda5d934ad8ecc1092659aefc1
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x0caf81986219be7bd2196cb7ae56de391c916176644e6bb5fe1788f4b6f4cf5f,
            0x09bf71a6496999ff9e1a3f1f48328ce884e0c20df2353ba59e707339a7899172
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x28d1e15fd500083babac5799cfa761160bc7d0be62e5e39c00eae91fa01b646d,
            0x02ff71de13c40d45d466c26edbf01199983d08b4e9ec5f1448ee52ad591b7b06
        );

         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x1aaae757fb24641005a750069741b8a5c2f52bb231efcf31ac6d49d2d294864d,
            0x11e38daed2d0858b9ec230253df119d784501cbf6e28043bccea2177b031ab28
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x1bc4bb99f60043163027a17a817deb7aa3e477bc905d2866c5bbd4c070587794,
            0x1ee030ff74407d4977f217af7de228207887f43efb09c781898b9d972c8fe58c
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x00cf72ca3414cf6cf636751d7b44ea0f63b08adc3b16e9e5abe5d1be5583e004,
            0x2773df14efcc4eccc70898888199033b1c80605f14aa15c4e528011b21a99d1d
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x1342d591669f321618e27fd31b4f80012521656b98391070fc851b8eba8568d2,
            0x1622e38a3a3f0465bd68929d6fab92e327b52d075fc35819c7d85c861874e3ca
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
        if (DUMMY_VERIFIER) {
            return true;
        }
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
