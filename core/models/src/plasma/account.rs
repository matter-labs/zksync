use crate::circuit;
use crate::plasma::params;
use crate::primitives::GetBits;
use crate::{Engine, Fr, PublicKey};
use bigdecimal::BigDecimal;
use fnv::FnvHashMap;
use sapling_crypto::jubjub::{edwards, Unknown};

pub type TokenId = u8;

pub const TOTAL_TOKENS: usize = 256;
pub const ETH_TOKEN_ID: TokenId = 0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    balances: Vec<BigDecimal>,
    pub nonce: u32,
    pub public_key_x: Fr,
    pub public_key_y: Fr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// TODO (Drogan) remove id from here up.
pub enum AccountUpdate {
    Create {
        id: u32,
        public_key_x: Fr,
        public_key_y: Fr,
        nonce: u32,
    },
    Delete {
        id: u32,
        public_key_x: Fr,
        public_key_y: Fr,
        nonce: u32,
    },
    UpdateBalance {
        id: u32,
        nonce: u32,
        // (token, old, new)
        balance_update: (TokenId, BigDecimal, BigDecimal),
    },
}

impl AccountUpdate {
    pub fn get_account_id(&self) -> u32 {
        match *self {
            AccountUpdate::Create { id, .. } => id,
            AccountUpdate::Delete { id, .. } => id,
            AccountUpdate::UpdateBalance { id, .. } => id,
        }
    }

    pub fn reverse_update(&self) -> Self {
        match self {
            AccountUpdate::Create {
                id,
                public_key_x,
                public_key_y,
                nonce,
            } => AccountUpdate::Delete {
                id: *id,
                public_key_x: public_key_x.clone(),
                public_key_y: public_key_y.clone(),
                nonce: *nonce,
            },
            AccountUpdate::Delete {
                id,
                public_key_x,
                public_key_y,
                nonce,
            } => AccountUpdate::Create {
                id: *id,
                public_key_x: public_key_x.clone(),
                public_key_y: public_key_y.clone(),
                nonce: *nonce,
            },
            AccountUpdate::UpdateBalance {
                id,
                nonce,
                balance_update,
            } => AccountUpdate::UpdateBalance {
                id: *id,
                nonce: *nonce,
                balance_update: (
                    balance_update.0,
                    balance_update.2.clone(),
                    balance_update.1.clone(),
                ),
            },
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            balances: vec![BigDecimal::default(); TOTAL_TOKENS],
            nonce: 0,
            public_key_x: Fr::default(),
            public_key_y: Fr::default(),
        }
    }
}

impl GetBits for Account {
    fn get_bits_le(&self) -> Vec<bool> {
        circuit::account::CircuitAccount::<Engine>::from(self.clone()).get_bits_le()

        // TODO: make more efficient:

        // let mut leaf_content = Vec::new();
        // leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        // leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        // leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content
    }
}

impl Account {
    pub fn get_pub_key(&self) -> Option<PublicKey> {
        let point = edwards::Point::<Engine, Unknown>::from_xy(
            self.public_key_x,
            self.public_key_y,
            &params::JUBJUB_PARAMS,
        );
        point.map(sapling_crypto::eddsa::PublicKey::<Engine>)
    }

    fn get_token(&self, token: TokenId) -> &BigDecimal {
        self.balances
            .get(usize::from(token))
            .expect("Token not found")
    }

    fn get_token_mut(&mut self, token: TokenId) -> &mut BigDecimal {
        self.balances
            .get_mut(usize::from(token))
            .expect("Token not found")
    }

    pub fn get_balance(&self, token: TokenId) -> &BigDecimal {
        self.get_token(token)
    }

    pub fn balances(&self) -> Vec<BigDecimal> {
        self.balances.clone()
    }

    pub fn set_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        std::mem::replace(self.get_token_mut(token), amount.clone());
    }

    pub fn add_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        *self.get_token_mut(token) += amount;
    }

    pub fn sub_balance(&mut self, token: TokenId, amount: &BigDecimal) {
        *self.get_token_mut(token) -= amount;
    }

    pub fn apply_update(account: Option<Self>, update: AccountUpdate) -> Option<Self> {
        match account {
            Some(mut account) => match update {
                AccountUpdate::Delete { .. } => None,
                AccountUpdate::UpdateBalance {
                    balance_update: (token, _, amount),
                    ..
                } => {
                    account.set_balance(token, &amount);
                    Some(account)
                }
                _ => {
                    error!(
                        "Incorrect update received {:?} for account {:?}",
                        update, account
                    );
                    Some(account)
                }
            },
            None => match update {
                AccountUpdate::Create {
                    public_key_x,
                    public_key_y,
                    ..
                } => {
                    let mut new_account = Account::default();
                    new_account.public_key_y = public_key_y;
                    new_account.public_key_x = public_key_x;
                    Some(new_account)
                }
                _ => {
                    error!("Incorrect update received {:?} for empty account", update);
                    None
                }
            },
        }
    }
}

#[test]
fn test_default_account() {
    let a = Account::default();
    a.get_bits_le();
}
