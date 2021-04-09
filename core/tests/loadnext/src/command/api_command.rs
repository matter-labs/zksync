use zksync_types::Address;

use crate::account_pool::AddressPool;

#[derive(Debug, Clone)]
pub enum ApiRequestCommand {}

impl ApiRequestCommand {
    pub fn random(_own_address: Address, _addresses: &AddressPool) -> Self {
        todo!()
    }
}
