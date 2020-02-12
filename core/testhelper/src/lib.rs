use rand::Rng;

/// TestAccount is an account with random generated keys and address.
pub struct TestAccount {
    pub private_key: franklin_crypto::eddsa::PrivateKey<pairing::bn256::Bn256>,
    pub public_key: franklin_crypto::eddsa::PublicKey<pairing::bn256::Bn256>,
    pub address: web3::types::Address,
}

// TODO: move to helper crate
impl TestAccount {
    pub fn new() -> Self {
        let rng = &mut rand::thread_rng();
        let p_g = franklin_crypto::alt_babyjubjub::FixedGenerators::SpendingKeyGenerator;
        let jubjub_params = &franklin_crypto::alt_babyjubjub::AltJubjubBn256::new();
        let private_key = franklin_crypto::eddsa::PrivateKey::<pairing::bn256::Bn256>(rng.gen());
        let address = rng.gen::<[u8; 20]>().into();
        let public_key = franklin_crypto::eddsa::PublicKey::<pairing::bn256::Bn256>::from_private(
            &private_key,
            p_g,
            jubjub_params,
        );
        TestAccount {
            private_key,
            public_key,
            address,
        }
    }
}

impl Default for TestAccount {
    fn default() -> Self {
        TestAccount::new()
    }
}
