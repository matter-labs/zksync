use num::BigUint;
use rand::Rng;

use zksync_types::Address;

use crate::{
    account_pool::AddressPool,
    all::{All, AllWeighted},
    rng::{LoadtestRng, WeightedRandom},
};

/// Type of transaction. It doesn't copy the zkSync operation list, because
/// it divides some transactions in subcategories (e.g. to new account / to existing account; to self / to other; etc)/
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TxType {
    Deposit,
    TransferToNew,
    TransferToExisting,
    WithdrawToSelf,
    WithdrawToOther,
    FullExit,
    ChangePubKey,
}

impl All for TxType {
    fn all() -> &'static [Self] {
        &[
            Self::Deposit,
            Self::TransferToNew,
            Self::TransferToExisting,
            Self::WithdrawToSelf,
            Self::WithdrawToOther,
            Self::FullExit,
            Self::ChangePubKey,
        ]
    }
}

impl AllWeighted for TxType {
    fn all_weighted() -> &'static [(Self, f32)] {
        // All available options together with their weight.
        // `TransferToNew` and `TransferToExisting` the most likely options.
        const DEFAULT_WEIGHT: f32 = 1.0;
        const HIGH_WEIGHT: f32 = 3.0;
        &[
            (Self::Deposit, DEFAULT_WEIGHT),
            (Self::TransferToNew, HIGH_WEIGHT),
            (Self::TransferToExisting, HIGH_WEIGHT),
            (Self::WithdrawToSelf, DEFAULT_WEIGHT),
            (Self::WithdrawToOther, DEFAULT_WEIGHT),
            (Self::FullExit, DEFAULT_WEIGHT),
            (Self::ChangePubKey, DEFAULT_WEIGHT),
        ]
    }
}

impl TxType {
    /// Generates a random transaction type that can be a part of the batch.
    pub fn random_batchable(rng: &mut LoadtestRng) -> Self {
        loop {
            let output = Self::random(rng);

            // Priority ops cannot be inserted into the batch.
            if output.is_batchable() {
                return output;
            }
        }
    }

    /// Checks whether `TxType` can be used as a part of the batch.
    fn is_batchable(self) -> bool {
        !matches!(self, Self::Deposit | Self::FullExit)
    }

    fn is_withdrawal(self) -> bool {
        matches!(self, Self::WithdrawToOther | Self::WithdrawToSelf)
    }

    fn is_change_pubkey(self) -> bool {
        matches!(self, Self::ChangePubKey)
    }

    fn is_priority(self) -> bool {
        matches!(self, Self::Deposit | Self::FullExit)
    }

    fn is_target_self(self) -> bool {
        matches!(self, Self::WithdrawToSelf | Self::FullExit)
    }
}

/// Modifier to be applied to the transaction in order to make it incorrect.
/// Incorrect transactions are a significant part of loadtest, because we want to ensure
/// that server is resilient for all the possible kinds of user input.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum IncorrectnessModifier {
    ZeroFee,
    IncorrectZkSyncSignature,
    IncorrectEthSignature,
    NonExistentToken,
    TooBigAmount,
    NotPackableAmount,
    NotPackableFeeAmount,

    // Last option goes for no modifier,
    // since it's more convenient than dealing with `Option<IncorrectnessModifier>`.
    None,
}

impl IncorrectnessModifier {
    // Have to implement this as a const function, since const functions in traits are not stabilized yet.
    const fn const_all() -> &'static [Self] {
        &[
            Self::ZeroFee,
            Self::IncorrectZkSyncSignature,
            Self::IncorrectEthSignature,
            Self::NonExistentToken,
            Self::TooBigAmount,
            Self::NotPackableAmount,
            Self::NotPackableFeeAmount,
            Self::None,
        ]
    }
}

impl All for IncorrectnessModifier {
    fn all() -> &'static [Self] {
        Self::const_all()
    }
}

impl AllWeighted for IncorrectnessModifier {
    fn all_weighted() -> &'static [(Self, f32)] {
        const VARIANT_AMOUNTS: f32 = IncorrectnessModifier::const_all().len() as f32;
        // No modifier is 9 times probable than all the other variants in sum.
        // In other words, 90% probability of no modifier.
        const NONE_PROBABILITY: f32 = (VARIANT_AMOUNTS - 1.0) * 9.0;
        const DEFAULT_PROBABILITY: f32 = 1.0f32;

        &[
            (Self::ZeroFee, DEFAULT_PROBABILITY),
            (Self::IncorrectZkSyncSignature, DEFAULT_PROBABILITY),
            (Self::IncorrectEthSignature, DEFAULT_PROBABILITY),
            (Self::NonExistentToken, DEFAULT_PROBABILITY),
            (Self::TooBigAmount, DEFAULT_PROBABILITY),
            (Self::NotPackableAmount, DEFAULT_PROBABILITY),
            (Self::NotPackableFeeAmount, DEFAULT_PROBABILITY),
            (Self::None, NONE_PROBABILITY),
        ]
    }
}

impl IncorrectnessModifier {
    fn affects_amount(self) -> bool {
        matches!(self, Self::TooBigAmount | Self::NotPackableAmount)
    }

    fn is_not_packable_amount(self) -> bool {
        matches!(self, Self::NotPackableAmount)
    }
}

/// Expected outcome of transaction:
/// Since we may create erroneous transactions on purpose,
/// we may expect different outcomes for each transaction.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ExpectedOutcome {
    /// Transactions was successfully executed.
    TxSucceed,
    /// Transaction sending should fail.
    ApiRequestFailed,
    /// Transaction should be accepted, but rejected at the
    /// time of execution.
    TxRejected,
}

impl IncorrectnessModifier {
    pub fn expected_outcome(self) -> ExpectedOutcome {
        match self {
            Self::None => ExpectedOutcome::TxSucceed,

            Self::ZeroFee
            | Self::IncorrectEthSignature
            | Self::IncorrectZkSyncSignature
            | Self::NonExistentToken
            | Self::NotPackableAmount
            | Self::NotPackableFeeAmount => ExpectedOutcome::ApiRequestFailed,

            Self::TooBigAmount => ExpectedOutcome::TxRejected,
        }
    }
}

/// Complete description of a transaction that must be executed by a test wallet.
#[derive(Debug, Clone)]
pub struct TxCommand {
    /// Type of operation.
    pub command_type: TxType,
    /// Whether and how transaction should be corrupted.
    pub modifier: IncorrectnessModifier,
    /// Recipient address.
    pub to: Address,
    /// Transaction amount (0 if not applicable).
    pub amount: BigUint,
}

impl TxCommand {
    pub fn change_pubkey(address: Address) -> Self {
        Self {
            command_type: TxType::ChangePubKey,
            modifier: IncorrectnessModifier::None,
            to: address,
            amount: 0u64.into(),
        }
    }

    /// Generates a fully random transaction command.
    pub fn random(rng: &mut LoadtestRng, own_address: Address, addresses: &AddressPool) -> Self {
        let command_type = TxType::random(rng);

        Self::new_with_type(rng, own_address, addresses, command_type)
    }

    /// Generates a random transaction command that can be a part of the batch.
    pub fn random_batchable(
        rng: &mut LoadtestRng,
        own_address: Address,
        addresses: &AddressPool,
    ) -> Self {
        let command_type = TxType::random_batchable(rng);

        Self::new_with_type(rng, own_address, addresses, command_type)
    }

    fn new_with_type(
        rng: &mut LoadtestRng,
        own_address: Address,
        addresses: &AddressPool,
        command_type: TxType,
    ) -> Self {
        let mut command = Self {
            command_type,
            modifier: IncorrectnessModifier::random(rng),
            to: addresses.random_address(rng),
            amount: Self::random_amount(rng),
        };

        // Check whether we should use a non-existent address.
        if matches!(command.command_type, TxType::TransferToNew) {
            command.to = Address::random();
        }

        // Check whether we should use a self as a target.
        if command.command_type.is_target_self() {
            command.to = own_address;
        }

        // Transactions that have no amount field.
        let no_amount_field =
            command.command_type.is_change_pubkey() && command.modifier.affects_amount();
        // It doesn't make sense to fail contract-based functions.
        let incorrect_priority_op = command.command_type.is_priority();
        // Amount doesn't have to be packable for withdrawals.
        let unpackable_withdrawal =
            command.command_type.is_withdrawal() && command.modifier.is_not_packable_amount();

        // Check whether generator modifier does not make sense.
        if no_amount_field || incorrect_priority_op || unpackable_withdrawal {
            command.modifier = IncorrectnessModifier::None;
        }

        command
    }

    fn random_amount(rng: &mut LoadtestRng) -> BigUint {
        rng.gen_range(0u64, 2u64.pow(18)).into()
    }
}
