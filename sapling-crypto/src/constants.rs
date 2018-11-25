/// First 64 bytes of the BLAKE2s input during group hash.
/// This is chosen to be some random string that we couldn't have anticipated when we designed
/// the algorithm, for rigidity purposes.
/// We deliberately use an ASCII hex string of 32 bytes here.
pub const GH_FIRST_BLOCK: &'static [u8; 64]
          = b"096b36a5804bfacef1691e173c366a47ff5ba84a44f26ddd7e8d9f79d5b42df0";

// BLAKE2s invocation personalizations
/// BLAKE2s Personalization for CRH^ivk = BLAKE2s(ak | nk)
pub const CRH_IVK_PERSONALIZATION: &'static [u8; 8]
          = b"Zcashivk";

/// BLAKE2s Personalization for PRF^nf = BLAKE2s(nk | rho)
pub const PRF_NF_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_nf";

// Group hash personalizations
/// BLAKE2s Personalization for Pedersen hash generators.
pub const PEDERSEN_HASH_GENERATORS_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_PH";

/// BLAKE2s Personalization for the group hash for key diversification
pub const KEY_DIVERSIFICATION_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_gd";

/// BLAKE2s Personalization for the spending key base point
pub const SPENDING_KEY_GENERATOR_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_G_";

/// BLAKE2s Personalization for the proof generation key base point
pub const PROOF_GENERATION_KEY_BASE_GENERATOR_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_H_";

/// BLAKE2s Personalization for the value commitment generator for the value
pub const VALUE_COMMITMENT_GENERATOR_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_cv";

/// BLAKE2s Personalization for the nullifier position generator (for computing rho)
pub const NULLIFIER_POSITION_IN_TREE_GENERATOR_PERSONALIZATION: &'static [u8; 8]
          = b"Zcash_J_";

/// BLAKE2s Personalization hash of (R_x || message) in EdDSA variant with 256 bit hash
pub const MATTER_EDDSA_BLAKE2S_PERSONALIZATION: &'static [u8; 8] 
            = b"Matter_H";