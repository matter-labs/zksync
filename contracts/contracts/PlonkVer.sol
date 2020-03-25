pragma solidity >=0.5.0 <0.7.0;

library PairingsBn254 {
    uint256 constant q_mod = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
    uint256 constant r_mod = 21888242871839275222246405745257275088548364400416034343698204186575808495617;
    
    struct G1Point {
        uint256 X;
        uint256 Y;
    } 
    
    struct Fr {
        uint256 value;
    }
    
    function new_fr(uint256 fr) internal pure returns (Fr memory) {
        require(fr < r_mod);
        return Fr({value: fr});
    }
    
    function copy(Fr memory self) internal pure returns (Fr memory n) {
        n.value = self.value;
    }
    
    function assign(Fr memory self, Fr memory other) internal pure {
        self.value = other.value;
    }
    
    function inverse(Fr memory fr) internal view returns (Fr memory) {
        assert(fr.value != 0);
        return pow(fr, r_mod-2);
    }
    
    function add_assign(Fr memory self, Fr memory other) internal pure {
        self.value = addmod(self.value, other.value, r_mod);
    }
    
    function sub_assign(Fr memory self, Fr memory other) internal pure {
        self.value = addmod(self.value, r_mod - other.value, r_mod);
    }
    
    function mul_assign(Fr memory self, Fr memory other) internal pure {
        self.value = mulmod(self.value, other.value, r_mod);
    }
    
    function pow(Fr memory self, uint256 power) internal view returns (Fr memory) {
        uint256[6] memory input = [32, 32, 32, self.value, power, r_mod];
        uint256[1] memory result;
        bool success;
        assembly {
            success := staticcall(gas(), 0x05, input, 0xc0, result, 0x20)
        }
        require(success);
        return Fr({value: result[0]});
    }
    
    // Encoding of field elements is: X[0] * z + X[1]
    struct G2Point {
        uint[2] X;
        uint[2] Y;
    }

    function P1() internal pure returns (G1Point memory) {
        return G1Point(1, 2);
    }
    
    function new_g1(uint256 x, uint256 y) internal pure returns (G1Point memory) {
        return G1Point(x, y);
    }
    
    function new_g2(uint256[2] memory x, uint256[2] memory y) internal pure returns (G2Point memory) {
        return G2Point(x, y);
    }
    
    function copy_g1(G1Point memory self) internal pure returns (G1Point memory result) {
        result.X = self.X;
        result.Y = self.Y;
    }

    function P2() internal pure returns (G2Point memory) {
        // for some reason ethereum expects to have c1*v + c0 form
        
        return G2Point(
            [0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2,
                0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed],
            [0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b,
                0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa]
        );
    }

    function negate(G1Point memory self) internal pure {
        // The prime q in the base field F_q for G1
        if (self.X == 0 && self.Y == 0)
            return;
        self.Y = q_mod - self.Y;
    }
    
    // function is_infinity(G1Point memory p) internal pure returns (bool) {
    //     if (p.X == 0 && p.Y == 0) {
    //         return true;
    //     }
    //     return false;
    // }

    function point_add(G1Point memory p1, G1Point memory p2)
        internal view returns (G1Point memory r)
    {
        point_add_into_dest(p1, p2, r);
        return r;
    }
    
    function point_add_assign(G1Point memory p1, G1Point memory p2)
        internal view
    {
        point_add_into_dest(p1, p2, p1);
    }

    function point_add_into_dest(G1Point memory p1, G1Point memory p2, G1Point memory dest)
        internal view
    {
        uint256[4] memory input;
        if (p2.X == 0 && p2.Y == 0) {
            // we add zero, nothing happens
            dest.X = p1.X;
            dest.Y = p1.Y;
            return;
        } else if (p1.X == 0 && p1.Y == 0) {
            // we add into zero, and we add non-zero point
            dest.X = p2.X;
            dest.Y = p2.Y;
            return;
        } else {
            input[0] = p1.X;
            input[1] = p1.Y;
            input[2] = p2.X;
            input[3] = p2.Y;
        }
        bool success = false;
        assembly {
            success := staticcall(gas(), 6, input, 0x80, dest, 0x40)
        }
        require(success);
    }
    
    function point_sub_assign(G1Point memory p1, G1Point memory p2)
        internal view
    {
        point_sub_into_dest(p1, p2, p1);
    }

    function point_sub_into_dest(G1Point memory p1, G1Point memory p2, G1Point memory dest)
        internal view
    {
        uint256[4] memory input;
        if (p2.X == 0 && p2.Y == 0) {
            // we subtracted zero, nothing happens
            dest.X = p1.X;
            dest.Y = p1.Y;
            return;
        } else if (p1.X == 0 && p1.Y == 0) {
            // we subtract from zero, and we subtract non-zero point
            dest.X = p2.X;
            dest.Y = q_mod - p2.Y;
            return;
        } else {
            input[0] = p1.X;
            input[1] = p1.Y;
            input[2] = p2.X;
            input[3] = q_mod - p2.Y;
        }
        bool success = false;
        assembly {
            success := staticcall(gas(), 6, input, 0x80, dest, 0x40)
        }
        require(success);
    }


    function point_mul(G1Point memory p, Fr memory s)
        internal view returns (G1Point memory r)
    {
        point_mul_into_dest(p, s, r);
        return r;
    }
    
    function point_mul_assign(G1Point memory p, Fr memory s)
        internal view
    {
        point_mul_into_dest(p, s, p);
    }

    function point_mul_into_dest(G1Point memory p, Fr memory s, G1Point memory dest)
        internal view
    {
        uint[3] memory input;
        input[0] = p.X;
        input[1] = p.Y;
        input[2] = s.value;
        bool success;
        assembly {
            success := staticcall(gas(), 7, input, 0x60, dest, 0x40)
        }
        require(success);
    }
    
    function pairing(G1Point[] memory p1, G2Point[] memory p2)
        internal view returns (bool)
    {
        require(p1.length == p2.length);
        uint elements = p1.length;
        uint inputSize = elements * 6;
        uint[] memory input = new uint[](inputSize);
        for (uint i = 0; i < elements; i++)
        {
            input[i * 6 + 0] = p1[i].X;
            input[i * 6 + 1] = p1[i].Y;
            input[i * 6 + 2] = p2[i].X[0];
            input[i * 6 + 3] = p2[i].X[1];
            input[i * 6 + 4] = p2[i].Y[0];
            input[i * 6 + 5] = p2[i].Y[1];
        }
        uint[1] memory out;
        bool success;
        assembly {
            success := staticcall(gas(), 8, add(input, 0x20), mul(inputSize, 0x20), out, 0x20)
        }
        require(success);
        return out[0] != 0;
    }

    /// Convenience method for a pairing check for two pairs.
    function pairingProd2(G1Point memory a1, G2Point memory a2, G1Point memory b1, G2Point memory b2)
        internal view returns (bool)
    {
        G1Point[] memory p1 = new G1Point[](2);
        G2Point[] memory p2 = new G2Point[](2);
        p1[0] = a1;
        p1[1] = b1;
        p2[0] = a2;
        p2[1] = b2;
        return pairing(p1, p2);
    }
}

library TranscriptLibrary {
    // flip                    0xe000000000000000000000000000000000000000000000000000000000000000;
    uint256 constant FR_MASK = 0x1fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff;

    uint32 constant DST_0 = 0;
    uint32 constant DST_1 = 1;
    uint32 constant DST_CHALLENGE = 2;
    
    struct Transcript {
        bytes32 state_0;
        bytes32 state_1;
        uint32 challenge_counter;
    }

    function new_transcript() internal pure returns (Transcript memory t) {
        t.state_0 = bytes32(0);
        t.state_1 = bytes32(0);
        t.challenge_counter = 0;
    }

    function update_with_u256(Transcript memory self, uint256 value) internal pure {
        bytes32 old_state_0 = self.state_0;
        self.state_0 = keccak256(abi.encodePacked(DST_0, old_state_0, self.state_1, value));
        self.state_1 = keccak256(abi.encodePacked(DST_1, old_state_0, self.state_1, value));
    }
    
    function update_with_fr(Transcript memory self, PairingsBn254.Fr memory value) internal pure {
        update_with_u256(self, value.value);
    }
    
    function update_with_g1(Transcript memory self, PairingsBn254.G1Point memory p) internal pure {
        update_with_u256(self, p.X);
        update_with_u256(self, p.Y);
    }
    
    function get_challenge(Transcript memory self) internal pure returns(PairingsBn254.Fr memory challenge) {
        bytes32 query = keccak256(abi.encodePacked(DST_CHALLENGE, self.state_0, self.state_1, self.challenge_counter));
        self.challenge_counter += 1;
        challenge = PairingsBn254.Fr({value: uint256(query) & FR_MASK});
    }
}

contract Plonk4VerifierWithAccessToDNext {
    using PairingsBn254 for PairingsBn254.G1Point;
    using PairingsBn254 for PairingsBn254.G2Point;
    using PairingsBn254 for PairingsBn254.Fr;
    
    using TranscriptLibrary for TranscriptLibrary.Transcript;

    uint256 constant STATE_WIDTH = 4;
    
    struct VerificationKey {
        uint256 domain_size;
        uint256 num_inputs;
        PairingsBn254.Fr omega;
        PairingsBn254.G1Point[STATE_WIDTH+2] selector_commitments; // STATE_WIDTH for witness + multiplication + constant
        PairingsBn254.G1Point[1] next_step_selector_commitments;
        PairingsBn254.G1Point[STATE_WIDTH] permutation_commitments;
        PairingsBn254.Fr[STATE_WIDTH-1] permutation_non_residues;
        PairingsBn254.G2Point g2_x;
    }
    
    struct Proof {
        uint256[] input_values;
        PairingsBn254.G1Point[STATE_WIDTH] wire_commitments;
        PairingsBn254.G1Point grand_product_commitment;
        PairingsBn254.G1Point[STATE_WIDTH] quotient_poly_commitments;
        PairingsBn254.Fr[STATE_WIDTH] wire_values_at_z;
        PairingsBn254.Fr[1] wire_values_at_z_omega;
        PairingsBn254.Fr grand_product_at_z_omega;
        PairingsBn254.Fr quotient_polynomial_at_z;
        PairingsBn254.Fr linearization_polynomial_at_z;
        PairingsBn254.Fr[STATE_WIDTH-1] permutation_polynomials_at_z;
    
        PairingsBn254.G1Point opening_at_z_proof;
        PairingsBn254.G1Point opening_at_z_omega_proof;
    }
    
    struct PartialVerifierState {
        PairingsBn254.Fr alpha;
        PairingsBn254.Fr beta;
        PairingsBn254.Fr gamma;
        PairingsBn254.Fr v;
        PairingsBn254.Fr u;
        PairingsBn254.Fr z;
        PairingsBn254.Fr[] cached_lagrange_evals;
    }
    
    function evaluate_lagrange_poly_out_of_domain(
        uint256 poly_num, 
        uint256 domain_size, 
        PairingsBn254.Fr memory omega, 
        PairingsBn254.Fr memory at
    ) internal view returns (PairingsBn254.Fr memory res) {
        require(poly_num < domain_size);
        PairingsBn254.Fr memory one = PairingsBn254.new_fr(1);
        PairingsBn254.Fr memory omega_power = omega.pow(poly_num);
        res = at.pow(domain_size);
        res.sub_assign(one);
        assert(res.value != 0); // Vanishing polynomial can not be zero at point `at`
        res.mul_assign(omega_power);
        
        PairingsBn254.Fr memory den = PairingsBn254.copy(at);
        den.sub_assign(omega_power);
        den.mul_assign(PairingsBn254.new_fr(domain_size));
        
        den = den.inverse();
        
        res.mul_assign(den);
    }
    
    function batch_evaluate_lagrange_poly_out_of_domain(
        uint256[] memory poly_nums, 
        uint256 domain_size, 
        PairingsBn254.Fr memory omega, 
        PairingsBn254.Fr memory at
    ) internal view returns (PairingsBn254.Fr[] memory res) {
        PairingsBn254.Fr memory one = PairingsBn254.new_fr(1);
        PairingsBn254.Fr memory tmp_1 = PairingsBn254.new_fr(0);
        PairingsBn254.Fr memory tmp_2 = PairingsBn254.new_fr(domain_size);
        PairingsBn254.Fr memory vanishing_at_z = at.pow(domain_size);
        vanishing_at_z.sub_assign(one);
        // we can not have random point z be in domain
        assert(vanishing_at_z.value != 0);
        PairingsBn254.Fr[] memory nums = new PairingsBn254.Fr[](poly_nums.length);
        PairingsBn254.Fr[] memory dens = new PairingsBn254.Fr[](poly_nums.length);
        // numerators in a form omega^i * (z^n - 1)
        // denoms in a form (z - omega^i) * N
        for (uint i = 0; i < poly_nums.length; i++) {
            tmp_1 = omega.pow(poly_nums[i]); // power of omega
            nums[i].assign(vanishing_at_z);
            nums[i].mul_assign(tmp_1);
            
            dens[i].assign(at); // (X - omega^i) * N
            dens[i].sub_assign(tmp_1); 
            dens[i].mul_assign(tmp_2); // mul by domain size
        }
        
        PairingsBn254.Fr[] memory partial_products = new PairingsBn254.Fr[](poly_nums.length);
        partial_products[0].assign(PairingsBn254.new_fr(1));
        for (uint i = 1; i < dens.length - 1; i++) {
            partial_products[i].assign(dens[i-1]);
            partial_products[i].mul_assign(dens[i]);
        }
    
        tmp_2.assign(partial_products[partial_products.length - 1]);
        tmp_2.mul_assign(dens[dens.length - 1]);
        tmp_2 = tmp_2.inverse(); // tmp_2 contains a^-1 * b^-1 (with! the last one)
        
        for (uint i = dens.length - 1; i < dens.length; i--) {
            dens[i].assign(tmp_2); // all inversed
            dens[i].mul_assign(partial_products[i]); // clear lowest terms    
            tmp_2.mul_assign(dens[i]);
        }
        
        for (uint i = 0; i < nums.length; i++) {
            nums[i].mul_assign(dens[i]);
        }

        return nums;
    }
    
    function evaluate_vanishing(
        uint256 domain_size, 
        PairingsBn254.Fr memory at
    ) internal view returns (PairingsBn254.Fr memory res) {
        res = at.pow(domain_size);
        res.sub_assign(PairingsBn254.new_fr(1));
    }
    
    function verify_at_z(
        PartialVerifierState memory state,
        Proof memory proof, 
        VerificationKey memory vk
    ) internal view returns (bool) {
        PairingsBn254.Fr memory lhs = evaluate_vanishing(vk.domain_size, state.z);
        assert(lhs.value != 0); // we can not check a polynomial relationship if point `z` is in the domain
        lhs.mul_assign(proof.quotient_polynomial_at_z);
    
        PairingsBn254.Fr memory quotient_challenge = PairingsBn254.new_fr(1);
        PairingsBn254.Fr memory rhs = PairingsBn254.copy(proof.linearization_polynomial_at_z);
        
        // public inputs
        PairingsBn254.Fr memory tmp = PairingsBn254.new_fr(0);
        for (uint256 i = 0; i < proof.input_values.length; i++) {
            tmp.assign(state.cached_lagrange_evals[i]);
            tmp.mul_assign(PairingsBn254.new_fr(proof.input_values[i]));
            rhs.add_assign(tmp);
        }
        
        quotient_challenge.mul_assign(state.alpha);
        
        PairingsBn254.Fr memory z_part = PairingsBn254.copy(proof.grand_product_at_z_omega);
        for (uint256 i = 0; i < proof.permutation_polynomials_at_z.length; i++) {
            tmp.assign(proof.permutation_polynomials_at_z[i]);
            tmp.mul_assign(state.beta);
            tmp.add_assign(state.gamma);
            tmp.add_assign(proof.wire_values_at_z[i]);
            
            z_part.mul_assign(tmp);
        }
        
        tmp.assign(state.gamma);
        // we need a wire value of the last polynomial in enumeration
        tmp.add_assign(proof.wire_values_at_z[STATE_WIDTH - 1]);
        
        z_part.mul_assign(tmp);
        z_part.mul_assign(quotient_challenge);
        
        rhs.sub_assign(z_part);
        
        quotient_challenge.mul_assign(state.alpha);
        
        tmp.assign(state.cached_lagrange_evals[0]);
        tmp.mul_assign(quotient_challenge);
        
        rhs.sub_assign(tmp);
        
        return lhs.value == rhs.value;
    }
    
    function reconstruct_d(
        PartialVerifierState memory state,
        Proof memory proof, 
        VerificationKey memory vk
    ) internal view returns (PairingsBn254.G1Point memory res) {
        // we compute what power of v is used as a delinearization factor in batch opening of 
        // commitments. Let's label W(x) = 1 / (x - z) *
        // [
            // t_0(x) + z^n * t_1(x) + z^2n * t_2(x) + z^3n * t_3(x) - t(z)
            // + v (r(x) - r(z))
            // + v^{2..5} * (witness(x) - witness(z))
            // + v^(6..8) * (permutation(x) - permutation(z))
        // ]
        // W'(x) = 1 / (x - z*omega) *
        // [
            // + v^9 (z(x) - z(z*omega)) <- we need this power
            // + v^10 * (d(x) - d(z*omega))
        // ]
        //
        // we pay a little for a few arithmetic operations to not introduce another constant
        uint256 power_for_z_omega_opening = 1 + 1 + STATE_WIDTH + STATE_WIDTH - 1;
        res = PairingsBn254.copy_g1(vk.selector_commitments[STATE_WIDTH + 1]);
                
        PairingsBn254.G1Point memory tmp_g1 = PairingsBn254.P1();
        PairingsBn254.Fr memory tmp_fr = PairingsBn254.new_fr(0);
        
        // addition gates
        for (uint256 i = 0; i < STATE_WIDTH; i++) {
            tmp_g1 = vk.selector_commitments[i].point_mul(proof.wire_values_at_z[i]);
            res.point_add_assign(tmp_g1);
        }
        
        // multiplication gate
        tmp_fr.assign(proof.wire_values_at_z[0]);
        tmp_fr.mul_assign(proof.wire_values_at_z[1]);
        tmp_g1 = vk.selector_commitments[STATE_WIDTH].point_mul(tmp_fr);
        res.point_add_assign(tmp_g1);
        
        // d_next
        tmp_g1 = vk.next_step_selector_commitments[0].point_mul(proof.wire_values_at_z_omega[0]);
        res.point_add_assign(tmp_g1);
        
        // z * non_res * beta + gamma + a
        PairingsBn254.Fr memory grand_product_part_at_z = PairingsBn254.copy(state.z);
        grand_product_part_at_z.mul_assign(state.beta);
        grand_product_part_at_z.add_assign(proof.wire_values_at_z[0]);
        grand_product_part_at_z.add_assign(state.gamma);
        for (uint256 i = 0; i < vk.permutation_non_residues.length; i++) {
            tmp_fr.assign(state.z);
            tmp_fr.mul_assign(vk.permutation_non_residues[i]);
            tmp_fr.mul_assign(state.beta);
            tmp_fr.add_assign(state.gamma);
            tmp_fr.add_assign(proof.wire_values_at_z[i+1]);
            
            grand_product_part_at_z.mul_assign(tmp_fr);
        }
        
        grand_product_part_at_z.mul_assign(state.alpha);
    
        tmp_fr.assign(state.cached_lagrange_evals[0]);
        tmp_fr.mul_assign(state.alpha);
        tmp_fr.mul_assign(state.alpha);
        
        grand_product_part_at_z.add_assign(tmp_fr);
        
        PairingsBn254.Fr memory grand_product_part_at_z_omega = state.v.pow(power_for_z_omega_opening);
        grand_product_part_at_z_omega.mul_assign(state.u);
        
        PairingsBn254.Fr memory last_permutation_part_at_z = PairingsBn254.new_fr(1);
        for (uint256 i = 0; i < proof.permutation_polynomials_at_z.length; i++) {
            tmp_fr.assign(state.beta);
            tmp_fr.mul_assign(proof.permutation_polynomials_at_z[i]);
            tmp_fr.add_assign(state.gamma);
            tmp_fr.add_assign(proof.wire_values_at_z[i]);
            
            last_permutation_part_at_z.mul_assign(tmp_fr);
        }

        last_permutation_part_at_z.mul_assign(state.beta);
        last_permutation_part_at_z.mul_assign(proof.grand_product_at_z_omega);
        last_permutation_part_at_z.mul_assign(state.alpha);
        
        // add to the linearization
        tmp_g1 = proof.grand_product_commitment.point_mul(grand_product_part_at_z);
        tmp_g1.point_sub_assign(vk.permutation_commitments[STATE_WIDTH - 1].point_mul(last_permutation_part_at_z));

        res.point_add_assign(tmp_g1);
        res.point_mul_assign(state.v);
        
        res.point_add_assign(proof.grand_product_commitment.point_mul(grand_product_part_at_z_omega));
    }
    
    function verify_commitments(
        PartialVerifierState memory state,
        Proof memory proof, 
        VerificationKey memory vk
    ) internal view returns (bool) {
        PairingsBn254.G1Point memory d = reconstruct_d(state, proof, vk);
        
        PairingsBn254.Fr memory z_in_domain_size = state.z.pow(vk.domain_size);
        
        PairingsBn254.G1Point memory tmp_g1 = PairingsBn254.P1();

        PairingsBn254.Fr memory aggregation_challenge = PairingsBn254.new_fr(1);
        
        PairingsBn254.G1Point memory commitment_aggregation = PairingsBn254.copy_g1(proof.quotient_poly_commitments[0]);
        PairingsBn254.Fr memory tmp_fr = PairingsBn254.new_fr(1);
        for (uint i = 1; i < proof.quotient_poly_commitments.length; i++) {
            tmp_fr.mul_assign(z_in_domain_size);
            tmp_g1 = proof.quotient_poly_commitments[i].point_mul(tmp_fr);
            commitment_aggregation.point_add_assign(tmp_g1);
        }

        aggregation_challenge.mul_assign(state.v);
        commitment_aggregation.point_add_assign(d);
        
        for (uint i = 0; i < proof.wire_commitments.length; i++) {
            aggregation_challenge.mul_assign(state.v);
            tmp_g1 = proof.wire_commitments[i].point_mul(aggregation_challenge);
            commitment_aggregation.point_add_assign(tmp_g1);
        }
        
        for (uint i = 0; i < vk.permutation_commitments.length - 1; i++) {
            aggregation_challenge.mul_assign(state.v);
            tmp_g1 = vk.permutation_commitments[i].point_mul(aggregation_challenge);
            commitment_aggregation.point_add_assign(tmp_g1);
        }
        
        aggregation_challenge.mul_assign(state.v);

        aggregation_challenge.mul_assign(state.v);

        tmp_fr.assign(aggregation_challenge);
        tmp_fr.mul_assign(state.u);
        tmp_g1 = proof.wire_commitments[STATE_WIDTH - 1].point_mul(tmp_fr);
        commitment_aggregation.point_add_assign(tmp_g1);
        
        // collect opening values
        aggregation_challenge = PairingsBn254.new_fr(1);
        
        PairingsBn254.Fr memory aggregated_value = PairingsBn254.copy(proof.quotient_polynomial_at_z);
        
        aggregation_challenge.mul_assign(state.v);

        tmp_fr.assign(proof.linearization_polynomial_at_z);
        tmp_fr.mul_assign(aggregation_challenge);
        aggregated_value.add_assign(tmp_fr);
        
        for (uint i = 0; i < proof.wire_values_at_z.length; i++) {
            aggregation_challenge.mul_assign(state.v);
            
            tmp_fr.assign(proof.wire_values_at_z[i]);
            tmp_fr.mul_assign(aggregation_challenge);
            aggregated_value.add_assign(tmp_fr);
        }
        
        for (uint i = 0; i < proof.permutation_polynomials_at_z.length; i++) {
            aggregation_challenge.mul_assign(state.v);

            tmp_fr.assign(proof.permutation_polynomials_at_z[i]);
            tmp_fr.mul_assign(aggregation_challenge);
            aggregated_value.add_assign(tmp_fr);
        }
        
        aggregation_challenge.mul_assign(state.v);

        tmp_fr.assign(proof.grand_product_at_z_omega);
        tmp_fr.mul_assign(aggregation_challenge);
        tmp_fr.mul_assign(state.u);
        aggregated_value.add_assign(tmp_fr);
        
        aggregation_challenge.mul_assign(state.v);

        tmp_fr.assign(proof.wire_values_at_z_omega[0]);
        tmp_fr.mul_assign(aggregation_challenge);
        tmp_fr.mul_assign(state.u);
        aggregated_value.add_assign(tmp_fr);
        
        commitment_aggregation.point_sub_assign(PairingsBn254.P1().point_mul(aggregated_value));
        
        PairingsBn254.G1Point memory pair_with_generator = commitment_aggregation;
        pair_with_generator.point_add_assign(proof.opening_at_z_proof.point_mul(state.z));
        
        tmp_fr.assign(state.z);
        tmp_fr.mul_assign(vk.omega);
        tmp_fr.mul_assign(state.u);
        pair_with_generator.point_add_assign(proof.opening_at_z_omega_proof.point_mul(tmp_fr));
        
        PairingsBn254.G1Point memory pair_with_x = proof.opening_at_z_omega_proof.point_mul(state.u);
        pair_with_x.point_add_assign(proof.opening_at_z_proof);
        pair_with_x.negate();
        
        return PairingsBn254.pairingProd2(pair_with_generator, PairingsBn254.P2(), pair_with_x, vk.g2_x);
    }
    
    function verify_initial(
        PartialVerifierState memory state, 
        Proof memory proof, 
        VerificationKey memory vk
    ) internal view returns (bool) {
        require(proof.input_values.length == vk.num_inputs);
        require(vk.num_inputs >= 1);
        TranscriptLibrary.Transcript memory transcript = TranscriptLibrary.new_transcript();
        for (uint256 i = 0; i < vk.num_inputs; i++) {
            transcript.update_with_u256(proof.input_values[i]);
        }
        
        for (uint256 i = 0; i < proof.wire_commitments.length; i++) {
            transcript.update_with_g1(proof.wire_commitments[i]);
        }
        
        state.beta = transcript.get_challenge();
        state.gamma = transcript.get_challenge();
        
        transcript.update_with_g1(proof.grand_product_commitment);
        state.alpha = transcript.get_challenge();
        
        for (uint256 i = 0; i < proof.quotient_poly_commitments.length; i++) {
            transcript.update_with_g1(proof.quotient_poly_commitments[i]);
        }
    
        state.z = transcript.get_challenge();
        
        uint256[] memory lagrange_poly_numbers = new uint256[](vk.num_inputs);
        for (uint256 i = 0; i < lagrange_poly_numbers.length; i++) {
            lagrange_poly_numbers[i] = i;
        }
        
        state.cached_lagrange_evals = batch_evaluate_lagrange_poly_out_of_domain(
            lagrange_poly_numbers,
            vk.domain_size, 
            vk.omega, state.z
        );

        bool valid = verify_at_z(state, proof, vk);

        if (valid == false) {
            return false;
        }
        
        for (uint256 i = 0; i < proof.wire_values_at_z.length; i++) {
            transcript.update_with_fr(proof.wire_values_at_z[i]);
        }
        
        for (uint256 i = 0; i < proof.wire_values_at_z_omega.length; i++) {
            transcript.update_with_fr(proof.wire_values_at_z_omega[i]);
        }
        
        for (uint256 i = 0; i < proof.permutation_polynomials_at_z.length; i++) {
            transcript.update_with_fr(proof.permutation_polynomials_at_z[i]);
        }
        
        transcript.update_with_fr(proof.quotient_polynomial_at_z);
        transcript.update_with_fr(proof.linearization_polynomial_at_z);
        
        state.v = transcript.get_challenge();
        transcript.update_with_g1(proof.opening_at_z_proof);
        transcript.update_with_g1(proof.opening_at_z_omega_proof);
        state.u = transcript.get_challenge();
        
        return true;
    }

    // This verifier is for a PLONK with a state width 4
    // and main gate equation
    // q_a(X) * a(X) + 
    // q_b(X) * b(X) + 
    // q_c(X) * c(X) + 
    // q_d(X) * d(X) + 
    // q_m(X) * a(X) * b(X) + 
    // q_constants(X)+ 
    // q_d_next(X) * d(X*omega)
    // where q_{}(X) are selectors a, b, c, d - state (witness) polynomials
    // q_d_next(X) "peeks" into the next row of the trace, so it takes 
    // the same d(X) polynomial, but shifted  
    
    function verify(Proof memory proof, VerificationKey memory vk) internal view returns (bool) {
        PartialVerifierState memory state;
        
        bool valid = verify_initial(state, proof, vk);
        
        if (valid == false) {
            return false;
        }
        
        valid = verify_commitments(state, proof, vk);
        
        return valid;
    }
}

contract ConcreteVerifier is Plonk4VerifierWithAccessToDNext {
    function get_verification_key() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = 4194304;
        vk.num_inputs = 1;
        vk.omega = PairingsBn254.new_fr(0x18c95f1ae6514e11a1b30fd7923947c5ffcec5347f16e91b4dd654168326bede);
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            0x2f702f46a651a9b388b5a908babecccc48e59306c80a75c2855ffe53ce1b60e7,
            0x278012543c718d110a97a29cf286c52330f9b780a57043cec08f36895309e4f1
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            0x305a8728721fad81716a0696f9afcde4f73211c108078a8b83f531d56456f7b1,
            0x05a1a823893642cf71ca251eaa7f9d7f1fe70ff2399b9ad84aa5a6e7b9419a4f
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            0x178ddfc4b5846bda8dcdc8a17bae74dbbc592a983de5b3b697e11b253f8a163b,
            0x2557ec9521f6e4ad15ef940cf76632a3cd4acc43c1534806804ee82b2ada918e
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            0x2892b4b8f0c46568d4c561e5b8b8566bd4377fdb12633f7f3c3f92cac610f896,
            0x2156898a3fa6eac08492441a9bb4a5a2db37aa392a10f034ae32bdfd2a17db45
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            0x11e132a7906d00e1e391b60c35dbcbf41f2dca61ba4e0c863d3bcd3c9cd34c4a,
            0x12e44baa9d800e085a5c6c5cae1f3e5b043327d8a12388c8c525ca942c0eef86
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            0x19c2f274c90b4fd99f74966980bdf7230437dc53fde5e3a1be30242566ff4db7,
            0x00e4105b033a7b6e2d1628a7949d9f14fb66421706cae56bef2c69b9d49f98c9
        );
        
        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            0x0676db98c4418f1a4f659a76e16b04a7c297df55250824a64952aaa0093070a0,
            0x1eb4f65fa2a9992c2465ceb19f42b030ce21b8fe006ace9b1262091c3e5d7c11
        );
        
         vk.permutation_commitments[0] = PairingsBn254.new_g1(
            0x2f4479997d6eca5143a476d491fecaeacec4b1422087ec8428e97e8127bdde19,
            0x07eb7a6bd6954fcb75e65304b75bcd732003caec057f444308b02d20802d291c
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            0x1629c2389d1008a4c2f806c5a853a003f7b7a7c45fcb869cc87c20d94a90bf3e,
            0x05ac17be842eb49f69711cb722a289f0c3cacb807e01cb17795bdefc92c5a99f
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            0x28afc12a8b5ff08374e6ef6494b09161ac089c06a161983ac621f1ed4d923b54,
            0x04da5a500aa46e058f2d64c62c713382c6c991b0009106b15e0ffec20f9c58a9
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            0x264adcb54404f3b255ad81b481edd4193f80decaa1b6fea28bbf5f29c36b1cc1,
            0x059b0a160706cda2cbc8e6a0d62d924e33a8a22aa8be8c66f70e1d0dbb88c470
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


    function deserialize_proof(
        uint256 expected_inputs, 
        uint256[] memory public_inputs, 
        uint256[] memory serialized_proof
    ) internal pure returns(Proof memory proof) {
        assert(expected_inputs == public_inputs.length);
        proof.input_values = new uint256[](expected_inputs);
        for (uint256 i = 0; i < expected_inputs; i++) {
            proof.input_values[i] = public_inputs[i];
        }
 
        uint256 j = 0;
        for (uint256 i = 0; i < STATE_WIDTH; i++) {
            proof.wire_commitments[i] = PairingsBn254.new_g1(
                serialized_proof[j],
                serialized_proof[j+1]
            );

            j += 2;
        }
        
        proof.grand_product_commitment = PairingsBn254.new_g1(
                serialized_proof[j],
                serialized_proof[j+1]
        );
        j += 2;
        
        for (uint256 i = 0; i < STATE_WIDTH; i++) {
            proof.quotient_poly_commitments[i] = PairingsBn254.new_g1(
                serialized_proof[j],
                serialized_proof[j+1]
            );

            j += 2;
        }
        
        for (uint256 i = 0; i < STATE_WIDTH; i++) {
            proof.wire_values_at_z[i] = PairingsBn254.new_fr(
                serialized_proof[j]
            );

            j += 1;
        }
        
        for (uint256 i = 0; i < proof.wire_values_at_z_omega.length; i++) {
            proof.wire_values_at_z_omega[i] = PairingsBn254.new_fr(
                serialized_proof[j]
            );

            j += 1;
        }
        
        proof.grand_product_at_z_omega = PairingsBn254.new_fr(
                serialized_proof[j]
            );

        j += 1;

        proof.quotient_polynomial_at_z = PairingsBn254.new_fr(
            serialized_proof[j]
        );

        j += 1;

        proof.linearization_polynomial_at_z = PairingsBn254.new_fr(
            serialized_proof[j]
        );

        j += 1;
    
        for (uint256 i = 0; i < proof.permutation_polynomials_at_z.length; i++) {
            proof.permutation_polynomials_at_z[i] = PairingsBn254.new_fr(
                serialized_proof[j]
            );

            j += 1;
        }

        proof.opening_at_z_proof = PairingsBn254.new_g1(
                serialized_proof[j],
                serialized_proof[j+1]
        );
        j += 2;

        proof.opening_at_z_omega_proof = PairingsBn254.new_g1(
                serialized_proof[j],
                serialized_proof[j+1]
        );
    
        j += 2;
        assert(j == serialized_proof.length);
    }
    
    function verify(
        uint256[] memory public_inputs, 
        uint256[] memory serialized_proof
    ) public view returns (bool) {
        VerificationKey memory vk = get_verification_key();
        uint256 expected_inputs = vk.num_inputs;

        Proof memory proof = deserialize_proof(expected_inputs, public_inputs, serialized_proof);

        bool valid = verify(proof, vk);

        return valid;
    }  
}