use crate::{QueryResult, StorageProcessor};
use zksync_types::{Address, ZkSyncTx};

pub fn address_to_stored_string(address: &Address) -> String {
    format!("0x{:x}", address)
}

pub fn stored_str_address_to_address(address: &str) -> Address {
    assert_eq!(address.len(), 42, "db stored token address length");
    address[2..]
        .parse()
        .expect("failed to parse stored db address")
}

pub async fn affected_accounts(
    tx: &ZkSyncTx,
    storage: &mut StorageProcessor<'_>,
) -> QueryResult<Vec<Address>> {
    let mut accounts = match tx {
        ZkSyncTx::Transfer(tx) => vec![tx.from, tx.to],
        ZkSyncTx::Withdraw(tx) => vec![tx.from, tx.to],
        ZkSyncTx::Close(tx) => vec![tx.account],
        ZkSyncTx::ChangePubKey(tx) => vec![tx.account],
        ZkSyncTx::ForcedExit(tx) => vec![tx.target],
        ZkSyncTx::Swap(tx) => {
            let mut accounts = vec![
                tx.submitter_address,
                tx.orders.0.recipient_address,
                tx.orders.1.recipient_address,
            ];
            if let Some(address) = storage
                .chain()
                .account_schema()
                .account_address_by_id(tx.orders.0.account_id)
                .await?
            {
                accounts.push(address);
            } else {
                anyhow::bail!("Order signer account id not found in db");
            }
            if let Some(address) = storage
                .chain()
                .account_schema()
                .account_address_by_id(tx.orders.1.account_id)
                .await?
            {
                accounts.push(address);
            } else {
                anyhow::bail!("Order signer account id not found in db");
            }
            accounts
        }
        ZkSyncTx::MintNFT(tx) => vec![tx.creator_address, tx.recipient],
        ZkSyncTx::WithdrawNFT(tx) => vec![tx.from, tx.to],
    };
    accounts.sort();
    accounts.dedup();
    Ok(accounts)
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn address_store_roundtrip() {
        let address = Address::random();
        let stored_address = address_to_stored_string(&address);
        assert_eq!(address, stored_str_address_to_address(&stored_address));
    }
}
