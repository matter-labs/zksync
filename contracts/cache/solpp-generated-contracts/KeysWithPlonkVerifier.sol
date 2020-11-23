pragma solidity >=0.5.0 <0.8.0;
pragma experimental ABIEncoderV2;




import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract KeysWithPlonkVerifier is VerifierWithDeserialize {
    uint256 constant VK_TREE_ROOT = 0x0040537a58863883e82f2b98e5e55b0074c2d9a7841cb60f86b69441df9b58b9;
    uint8 constant VK_MAX_INDEX = 0;

    function getVkAggregated(uint32 _blocks) internal pure returns (VerificationKey memory vk) {
        if (_blocks == uint32(1)) {
            return getVkAggregated1();
        } else if (_blocks == uint32(5)) {
            return getVkAggregated5();
        } else if (_blocks == uint32(10)) {
            return getVkAggregated10();
        } else if (_blocks == uint32(20)) {
            return getVkAggregated20();
        } else if (_blocks == uint32(40)) {
            return getVkAggregated40();
        }
    }

    function getVkExit() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 4194304;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x18c95f1ae6514e11a1b30fd7923947c5ffcec5347f16e91b4dd654168326bede);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x2c8d451dcd643bf943f7cea8843934f953ee3c4ed603feb35bc1f44dc7a4645a,
            0x133cf5f4874b61347b94e1e0449e0840171122d1726443561d14609ca65e1e30
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x18c25170185264607853f87404306ed9cc576eb2a427a473d18fcad16730efd2,
            0x12b1cd465d7fb9a799f3caefc72530769f09a4e2006b88a29e15dfc1fca4ddbb
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x2f1b858688a2cf447642b2ceb6fa9b7260f720a438db950e0ecf136ec03becae,
            0x1a7d199c15aba68da5e04081d333ca42b40aaaf130a2695c032b7bb6a8c6e423
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x1d4c4f40b2312edb75ab253075d32ec205deaf364822814b40e02ed1fd768f8b,
            0x28e21ecce75ceaefb583030a8073bd62b19bc403acaedcc3e2638065d7dc1373
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x10fe0197751aaea6547b367619d92d4485e93b5f9a3e5eac9c206c0d8fc468f8,
            0x2223144966dcb055d5adb6431c391ffb59166da83dda837584a1decf82ac12a8
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x1320e2858a0ae2ebc77110dfb37b130756be77fe417f8e814a60af99bd145f58,
            0x03053218b3925ad8c73a8ebfd0a67b268a8d8524869a54698a5a7d6f96941351
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x08db6ca8c199c9c909e6c5ac458c50f88e8d69a8322aa5ee14c972b8cd60c267,
            0x2f2004b0e750e205cd1fa89497ace1a0566865a215e8f61ac2a95ac30bd3ab0e
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x0c3163ba80868872736eb5d879ec7ae57846e89ea72b50f9a6f661907c851c7c,
            0x07893188d2decdec8263412cabcf9755160cd4df73cea9bff5f1e36e2b16de41
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x03e1dfef95d570f32aac88258cdb97934a52a7e264b9dfe1abb000066ecc8378,
            0x0d70213b80c79d28893b311ed41e30cb5caed7c2e91ce20001e415076a7190ed
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x1b2aa5532784995a44501ca6a5ef3f1b662338f1c951382e960b6348de6cfe36,
            0x26ed77297bd1ae364e6000dfe51bb4601825374e70e458bbb764bd7a2008b4bd
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x2742078792f67c82bbf2dd0305c4dbf2caec8d87639ad6ce904ecd666a7582be,
            0x28c1b55b2266b3cddb9a3052e1e00da0131982dfb46502405b97966007d92fe8
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x21ad71095dfe5ca99e3fe23c0eb43d8cdb322a422107fbe733ef091dd4ac8015,
            0x1241340e6e2740e73e927a09bd8b85ad77bd502ff595913ae0493d4877a1f37d
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x293a5669bb0ef21f2be8cd89d1c18c53660bdad3819713325aea75e3d8fb00d7,
            0x21f38a257cd069c38af3b6dfdc0d7f7f6d398955b98d7f8e5efd29e16a9057e0
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }

    function getVkAggregated1() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 4194304;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x18c95f1ae6514e11a1b30fd7923947c5ffcec5347f16e91b4dd654168326bede);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x2c8d451dcd643bf943f7cea8843934f953ee3c4ed603feb35bc1f44dc7a4645a,
            0x133cf5f4874b61347b94e1e0449e0840171122d1726443561d14609ca65e1e30
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x18c25170185264607853f87404306ed9cc576eb2a427a473d18fcad16730efd2,
            0x12b1cd465d7fb9a799f3caefc72530769f09a4e2006b88a29e15dfc1fca4ddbb
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x2f1b858688a2cf447642b2ceb6fa9b7260f720a438db950e0ecf136ec03becae,
            0x1a7d199c15aba68da5e04081d333ca42b40aaaf130a2695c032b7bb6a8c6e423
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x1d4c4f40b2312edb75ab253075d32ec205deaf364822814b40e02ed1fd768f8b,
            0x28e21ecce75ceaefb583030a8073bd62b19bc403acaedcc3e2638065d7dc1373
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x10fe0197751aaea6547b367619d92d4485e93b5f9a3e5eac9c206c0d8fc468f8,
            0x2223144966dcb055d5adb6431c391ffb59166da83dda837584a1decf82ac12a8
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x1320e2858a0ae2ebc77110dfb37b130756be77fe417f8e814a60af99bd145f58,
            0x03053218b3925ad8c73a8ebfd0a67b268a8d8524869a54698a5a7d6f96941351
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x08db6ca8c199c9c909e6c5ac458c50f88e8d69a8322aa5ee14c972b8cd60c267,
            0x2f2004b0e750e205cd1fa89497ace1a0566865a215e8f61ac2a95ac30bd3ab0e
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x0c3163ba80868872736eb5d879ec7ae57846e89ea72b50f9a6f661907c851c7c,
            0x07893188d2decdec8263412cabcf9755160cd4df73cea9bff5f1e36e2b16de41
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x03e1dfef95d570f32aac88258cdb97934a52a7e264b9dfe1abb000066ecc8378,
            0x0d70213b80c79d28893b311ed41e30cb5caed7c2e91ce20001e415076a7190ed
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x1b2aa5532784995a44501ca6a5ef3f1b662338f1c951382e960b6348de6cfe36,
            0x26ed77297bd1ae364e6000dfe51bb4601825374e70e458bbb764bd7a2008b4bd
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x2742078792f67c82bbf2dd0305c4dbf2caec8d87639ad6ce904ecd666a7582be,
            0x28c1b55b2266b3cddb9a3052e1e00da0131982dfb46502405b97966007d92fe8
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x21ad71095dfe5ca99e3fe23c0eb43d8cdb322a422107fbe733ef091dd4ac8015,
            0x1241340e6e2740e73e927a09bd8b85ad77bd502ff595913ae0493d4877a1f37d
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x293a5669bb0ef21f2be8cd89d1c18c53660bdad3819713325aea75e3d8fb00d7,
            0x21f38a257cd069c38af3b6dfdc0d7f7f6d398955b98d7f8e5efd29e16a9057e0
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }

    function getVkAggregated5() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 8388608;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1283ba6f4b7b1a76ba2008fe823128bea4adb9269cbfd7c41c223be65bc60863);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x1f024c4cfc23ef32a35cf95b62302492ec44a7c86d543c26cb2a4eadb582b624,
            0x04c212ff5f73b0337719a37ca9121e057807e382f6a6fce455ec89ec22538ea0
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x272eae1acc03b57c5ce3b3dbc2322ee408a530ccab11863c633a13e87af4c36b,
            0x08aedf6fea7318153401893cacc4746aab3944b4c43592019b7bbd16c3c3907b
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x0cd1158161c796aa0bbb63e7ba901994cdede7f70a0c31c514bf39f69b914009,
            0x2c8f2381841035e754977866e9914681ce617d319c45400639eb0484296e15e9
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x035cd62e471dd4f5828f94edee995df0c8b5ce191fff2d764dbd05e4b1672a37,
            0x251ddcc407b342c6261d5ad2ac33958fc2a1d0c9c04d0660f408456d10063213
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x0d6c7c6b06b32c392a8ed7bbe4df7584bf3d9856af760c95cd9a2787610bb3b2,
            0x2f479cebbec2be5855595ed26456a6cb1a053bb981ffd5c20f9f030091ed5cf2
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x285715f37edd047f6345dbc0e8d0af394d28b4e96cd666d0c27c2067fa186cc4,
            0x042b22efa2804d4022b9dd7a33cbf61f59897ccf2be0ffc3f61cce3e111dfec3
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x2b45983ad32902205f01bd0ae50b6293d888f7c6b43d95e0adcbb861b12fd83c,
            0x1da439855b9e8fc984048932d97cabfa343b02bb19045ceed5d7d52679f7628e
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x0d61bcaa302364562c49aa90e104173331a37191c760aa553404a66405309f0b,
            0x0665d237ea593acee3af259f9659e3ee9705e89e588fbce6c30a0aa07681e974
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x19c2feff3e9ea3863143fcf0773fdd64bbdd8525acd1cd7278f4af67b2e5389f,
            0x24f425b8325aac66be84164d2bf79601baaffe9206c638684b25db67f5dd1760
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x1ea016e1bd4e8b826756225634b47b5b1a132c192f201d0eb3fb91c23af7795f,
            0x0885316b36d58dba002313606bfafdafd5b2ce3f8bb00d5704f0fc966a210d86
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x2ef75c2b6f1d66ba85dd1658e51b6c0200c59361dcdfe4966c371f9ff2657d8b,
            0x00d11a97a26fae893e11beb424195036abd553ae65f436f4252ded2151d5c4e5
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x03e20cb4896a108038d5255e0a7d772b349c552aab18bb3d64c0fa08ce032b25,
            0x25d1f8d738e50604d75f91e92e8b451424f74fa0654d8d904986d010be988463
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x22b48eee6b736c48cb16aa66e6c660e685e4b17622b44a2d2dc24579c43402ae,
            0x1e87f495bc493baaf19ae4b3be2a2511a948cdd9e313ff5552cbff1578607d27
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }

    function getVkAggregated10() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 16777216;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1951441010b2b95a6e47a6075066a50a036f5ba978c050f2821df86636c0facb);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x2c6fb0fd7405044f5ae0acd13e0beec59f55db8718a6a5a37f21dd38cab5a169,
            0x2452bdd4b8abfcb618df76d097d6fb6f69dd0c6178e237ad4da92bf386162a94
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x2932b281f8db24ee84f170bf4b309749931f128f0bed1d6baaaab38b04ed588c,
            0x20c4b70358f33c095cfa9886269340b139feca833ca4181b389e6e9a38306fb0
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x293b159ae8d8e6b269f1a117b5946ea3d4030f58db08e5ce1591a216a625b8ec,
            0x222a805979ed94defa5f48cb9f0eeb55d55832120eabb31f87c7e2668d465f59
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x1a14b8ba7febd9f5142d2041a706426693325039e6d4d17ca51eaba292806c1a,
            0x1e22671c61578b8d6eec3ff0ae43c3f7e7f3c0fbff7c2a4c11e769edbafcb38b
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x1a6f7847f227789c4754e9479510976aa5e1342b7208773fd784beafcb6cb20b,
            0x2c370ddb50271677573cf932ee9eaed799295fb183f1846e83117f39db28f04d
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x0e7ec141b347c8fa707e9f476f2bc927f0608b6965f4019965c19ab0207edc30,
            0x15795af02b2feda16baee986681f70520426af30f24a8eae3f9b5fa9451427d8
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x07349f50fd3087c4bf86625fe76d2176c6e55c6c976ca8cc3d22b8ea472e9d28,
            0x2af5bc5a5c4411a277384835ed7db5e59fa920ddf0db8bc43a8ce5ca05c864c5
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x134e83d86f0222093e05c39371b6fd1d9b7ffad02ec30e67d1c7815fa154d3af,
            0x11dffbf423562a2a65b95610339bae7dfa37d302a68232aef659411f1f82a54a
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x1a76533af9b6199391dc8b03ce4d79718a4d91db3dbb97f40c9bc5e1f61015f3,
            0x1979af2a7794033786d1db7096444ceafe40b8aa8304735d43700abdcdbdf11d
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x02f6f51c35ebba8aad1fa6f24dbdb757c0e1c3415607483e0033a6fae70b0c3a,
            0x0ee7559170aa6db2a1ad0fa7c240fe04c7d135fb662cdf8baa4a1ae2b6b46e25
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x203d6b85c5380f179d1e58c5eee1443bdf6b03ffc32e8c14f7267eb3a1f76484,
            0x06d9aba1ef111ccd40ff26ea25ff800350ae6ed22b33f3bdd1e148ef09993e26
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x241337032b391d67a3012ab3ab41a2f867286bfcb7a1d32049338935f6e3da01,
            0x23c595a3b8675f0c571a05c936552ee4ffafc06fd5c43ed50eb160a90d444688
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x126e711caa0b7fd7f0e973cc8757b20ac47330e0376032b2ab213ec17c6a87f2,
            0x1b49e5f6a3c10d6b0061d1c13288f67db09e5f480ac246269d426fa7922c05d0
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }

    function getVkAggregated20() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 33554432;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x0d94d63997367c97a8ed16c17adaae39262b9af83acb9e003f94c217303dd160);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x0ece437fd64391e5ae36341ebbe444a88f82ce028c7b0c50a70515c48505d583,
            0x15b056e44b2941e1aa776ea0103b34175c447424c381b349eb67e8ecf11a4259
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x167344fe50565c45b6659e0aaf8caf574c909178c06a29f26482b71052809480,
            0x2eed8696a1ca301f7600ac0e7a9601c70ff1b6648ee2f2e5be96a0d1b7626cdc
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x294144e65fd1bc5aa755e39b9af0c61cbabe142fd2061e81f51ca38849dbe9bc,
            0x080bae29b87d1b007b05e08e551106da65adf0638466e8d558922e0b53fbbe15
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x2c571bbdf386ebb50475fe0e6651ada6ae4a4c6dea31df75e484e4b3c8abcb67,
            0x138325edb0f2a0e85d85006cfa8d9d395c44fe50dea61a173d8bb6ad52c89b17
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x2c62bc7d31128836666b97f55c12cfa8c564a886ff79421e97843bd205eeca61,
            0x2d9b9a225c4a937a947ddde8a679f2d875983853b153847f6c3831116ac880f1
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x1e4bc284c7b52928d2666c9bf58c0a33c398fa754e3379697548fe423dfdb6b2,
            0x29420b7796144068342b60cfbfdad14a269d0ea1dd2f10a283bdcf93224ebdf4
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x21bf86d63e11e165f24acfa451f989f6020f744a1c141d60fa76512d1b950ab0,
            0x007183abc01c3775d669a191dd1cc773e7be98064a09fd6eeb189483640015c8
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x2c0da680ac8c87196a2ac9a41cc3eb908fe3fc039befa4924372ec8e8e2c9130,
            0x19c4065926d0637e3679ba54f1e1fbef7fc281ae175f65fd31f2507a19a0191a
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x1cc441b5c21afd0bb9443f1270f0d6bf6d998404cb4e9af2acb9fd956944cee1,
            0x2a9ad7c26cd91e0b036a8d7d7f834098dcc71182499211ff927f252ba80d4b79
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x1b0d7e66bd42ffda6b881587fbda7844e9e78e94501f00b0831b37dc7abe9333,
            0x2e3b09086eae2e76834b07bbc86bc87fcff70d81061d592560222de54b693468
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x01508d42a521990060a61c06cddb6d62848f05bd8fe1b862c85fa741e10eb626,
            0x21e91770d830236a90a07425c9eb948a0ef8f85fa1fa2bfc359437c2958a9cd9
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x278e67050c3b2e32c6d67128e04c2fbd994b1e29be94ae27c6c8cd72e8e840e6,
            0x01cc853d48f2d2bc5f2b458de80466aac15613a4bb385d9fab37daafbeafdf10
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x20d89420786da547e20b3c9f571b94c66d568576b1dfd77c686464951d2a2b6b,
            0x12ffc3dfd82e1df5b13df512833ee73b4f77ba227963e6ea1d0184ed63418449
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }

    function getVkAggregated40() internal pure returns (VerificationKey memory vk) {
        vk.domain_size = 67108864;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x1dba8b5bdd64ef6ce29a9039aca3c0e524395c43b9227b96c75090cc6cc7ec97);
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            0x201399789f089d8efe1b09fd322d2eb3160f1b58ed0a7be37081cead8566aa3a,
            0x0cfda7f9633a754f94feb62fc73558c8f72305a1cda9c1ed225ad9bf31ad0ee4
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            0x2c35869329b41f8a4348e0570f27abfdd2a5450119c4eeaa85b617107a096775,
            0x16c44cdf843b7e534e2ed21e0aedcfb837831963f075fc04271d341363645f8e
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            0x19cfed05a378298cee01d71a9f36ff35bfc44aeb02aab7c63c15e2c8daa2335b,
            0x2f5b5128ba9092e6178ae15173c5fe7c7493f988f9a8e70615c912cb4dce4e8c
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            0x18fa4383c94e51b75b66720706825c9829e89e9c2ef84d18110a5f579a6908a2,
            0x06e39d9f17bbe28c0fa4b5229fe392036c8ad082c5de0ad78e6cdf0a70ca4dce
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            0x11ea5e7912798125ce752803370fd48b3ca92331f77b507cd8ab2eda70840f97,
            0x1dc1ceeb28749f58515f1ddec6b9ddc7f9c66d11c1fd9e9cef79d6dfed47b253
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            0x2238958e4809973dcfd9796d6060df38bc1d9af7f408d4b2ee5f333a07a83d7a,
            0x0c0b3115e3ca33f5f2f8f450c46b0ed9e04e9ca3143e9389dcbc860b2cce15ea
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            0x17d2b986e73d50e5543fee0c1c32489daf8c7b145a0b0c28d446628825c91e36,
            0x106857355a4272ec48c6c99668e75939634d66e711e4f8c673b2c44ecdcb1add
        );

        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            0x0fabd662151b37c46c47f05e06a3895364def6ee3c6f81ff0fefe0c794c37bde,
            0x1ce0e65bc2125f550d4c4e6315e6949e4b4027b79802c05c91bb76fe287e6799
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            0x29baea809973acc19c797c68d94458f42287ef8f5fb13b60d9b166faac0306c5,
            0x0a2113e8d5eb907cd6b38c7e3c9fb9c27792a666062e895054438e31e9e998f1
        );

        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            0x1d07eb43dc0a7c9b76e18c74b065e64ba0f04c70f865d447fad098f5fd839dba,
            0x25a4151062c1734e16f818cc018e2800126d0b91b8c1b31268dc297082bf7484
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            0x0ac118f7ec66d707c99b1b2ef82e487c6c9378affc6a6b90e2501fc2e59b3791,
            0x228742cd484240ad56cb12239d21af57ee2d2a76c4fc2a9f187428f598e29507
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            0x0239fbfcb99ded89dc704d4ab13e9240d28337eaf05108c6a1a310b52aff6f55,
            0x1672f6b66a756ceba54e48a8f6afcec7f651f0573068ea3537e71f337c81f9f6
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            0x12934c39a3febfda374d3857c446104a82732426cf284bb572208ce6ab436c5c,
            0x280a1a5d7b36420956ca6579d5d24752c14c0dcb0a28a1b7ebf2ad63e555e1cb
        );

        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000005
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            0x0000000000000000000000000000000000000000000000000000000000000007
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            0x000000000000000000000000000000000000000000000000000000000000000a
        );

        vk.g2_x = PairingsBn254.new_g2(
            [
                0x260e01b251f6f1c7e7ff4e580791dee8ea51d87a358e038b4efe30fac09383c1,
                0x0118c4d5b837bcc2bc89b5b398b5974e9f5944073b32078b7e231fec938883b0
            ],
            [
                0x04fc6369f7110fe3d25156c1bb9a72859cf2a04641f99ba4ee413c80da6a5fe4,
                0x22febda3c0c0632a56475b4214e5615e11e6dd3f96e6cea2854a87d4dacc5e55
            ]
        );
    }
}
