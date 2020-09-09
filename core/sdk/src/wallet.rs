// Wallet: TODO describe what's here
use crate::{provider::Provider, signer::Signer};
use anyhow;
use crypto_exports::rand::{thread_rng, Rng};
use models::node::priv_key_from_fs;
use models::node::tx::{PackedEthSignature, TxHash};
use models::node::{Address, FranklinTx, Nonce, TokenId, H256};
use num::BigUint;

#[derive(Debug)]
struct Wallet {
    pub provider: Provider,
    pub signer: Signer, // NOTE: Address in Wallet is the same as Address in provider
    pub address: Address,
}

// NOTE: This was was not strictly copy paste from elsewhere, deserves higher scrutiny
impl Wallet {
    fn new(provider: Provider, signer: Signer) -> Self {
        let signer_address = signer.address.clone();
        Wallet {
            provider,
            signer,
            address: signer_address,
        }
    }

    //Derive address from eth_pk, derive signer from signature of login message using eth_pk as a seed.
    pub async fn new_from_eth_private_key(eth_pk: H256, provider: Provider) -> Self {
        // NOTE: I may have misinterpreted how to derive zksync privkey. Borrowed this approach from zkSyncAccount::rand.
        let rng = &mut thread_rng();
        let zksync_pk = priv_key_from_fs(rng.gen());
        let address = PackedEthSignature::address_from_private_key(&eth_pk)
            .expect("private key is incorrect");
        // TODO: refactor error handling
        let acct_state = provider
            .account_state_info(address)
            .await
            .expect("request ");

        let nonce;
        match acct_state.id {
            None => nonce = 0,
            Some(id) => nonce = id,
        }

        let signer = Signer::new(zksync_pk, nonce, address, eth_pk);
        Self::new(provider, signer)
    }

    // Sign transaction with a signer and sumbit it using provider.
    #[allow(clippy::too_many_arguments)]
    pub async fn transfer(
        &self,
        token_id: TokenId,
        token_symbol: &str,
        amount: BigUint,
        fee: BigUint,
        to: &Address,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> Result<TxHash, anyhow::Error> {
        let (tx, eth_signature) = &self.signer.sign_transfer(
            token_id,
            token_symbol,
            amount,
            fee,
            to,
            nonce,
            increment_nonce,
        );
        // Clone, since behind a shared reference
        let franklin_tx = FranklinTx::Transfer(Box::new(tx.clone()));
        let ets = Some(eth_signature.clone());
        self.provider.send_tx(franklin_tx, ets).await
    }
}
