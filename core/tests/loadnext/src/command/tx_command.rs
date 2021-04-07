use num::BigUint;
use rand::{seq::SliceRandom, Rng};

use zksync_types::Address;

use crate::{account_pool::AddressPool, rng::LoadtestRng};

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

impl TxType {
    /// Generates a random transaction type. Not all the variants have the equal chance to be generated;
    /// specifically transfers are made more likely.
    pub fn random(rng: &mut LoadtestRng) -> Self {
        // All available options together with their weight.
        // `TransferToNew` and `TransferToExisting` the most likely options.
        const DEFAULT_WEIGHT: usize = 1;
        const HIGH_WEIGHT: usize = 3;
        let options = vec![
            (Self::Deposit, DEFAULT_WEIGHT),
            (Self::TransferToNew, HIGH_WEIGHT),
            (Self::TransferToExisting, HIGH_WEIGHT),
            (Self::WithdrawToSelf, DEFAULT_WEIGHT),
            (Self::WithdrawToOther, DEFAULT_WEIGHT),
            (Self::FullExit, DEFAULT_WEIGHT),
            (Self::ChangePubKey, DEFAULT_WEIGHT),
        ];

        // Now we can get weighted element by simply choosing the random value from the vector.
        options.choose_weighted(rng, |item| item.1).unwrap().0
    }

    /// Generates a random transaction type that can be a part of the batch.
    pub fn random_batchable(rng: &mut LoadtestRng) -> Self {
        loop {
            let output = Self::random(rng);

            // Priority ops and ChangePubKey cannot be inserted into the batch.
            if !matches!(output, Self::Deposit | Self::FullExit | Self::ChangePubKey) {
                return output;
            }
        }
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
    pub fn random(rng: &mut LoadtestRng) -> Self {
        if Self::no_modifier(rng) {
            return Self::None;
        }

        let options = &[
            Self::ZeroFee,
            Self::IncorrectZkSyncSignature,
            Self::IncorrectEthSignature,
            Self::NonExistentToken,
            Self::TooBigAmount,
            Self::NotPackableAmount,
            Self::NotPackableFeeAmount,
        ];

        options.choose(rng).copied().unwrap()
    }

    fn no_modifier(rng: &mut LoadtestRng) -> bool {
        // 90% of transactions should be correct.
        const NO_MODIFIER_PROBABILITY: f32 = 0.9f32;

        let chance = rng.gen_range(0f32, 1f32);

        chance <= NO_MODIFIER_PROBABILITY
    }

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

        // Check whether we should use a self as an target.
        if matches!(
            command.command_type,
            TxType::WithdrawToSelf | TxType::FullExit
        ) {
            command.to = own_address;
        }

        // Transactions that have no amount field.
        let no_amount_field = matches!(command.command_type, TxType::ChangePubKey)
            && matches!(
                command.modifier,
                IncorrectnessModifier::TooBigAmount | IncorrectnessModifier::NotPackableAmount
            );
        // It doesn't make sense to fail contract-based functions.
        let incorrect_priority_op =
            matches!(command.command_type, TxType::Deposit | TxType::FullExit);
        // Amount doesn't have to be packable for withdrawals.
        let unpackable_withdrawal = matches!(
            command.command_type,
            TxType::WithdrawToOther | TxType::WithdrawToSelf
        ) && command.modifier
            == IncorrectnessModifier::NotPackableAmount;

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
