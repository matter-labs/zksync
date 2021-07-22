use std::time::Duration;

use zksync_types::Address;

use crate::{
    all::All,
    command::{ApiRequestCommand, Command, TxType},
    constants::MAX_BATCH_SIZE,
};

/// Report for any operation done by loadtest.
///
/// Reports are yielded by `Executor` or `AccountLifespan` and are collected
/// by the `ReportCollector`.
///
/// Reports are expected to contain any kind of information useful for the analysis
/// and deciding whether the test was passed.
#[derive(Debug, Clone)]
pub struct Report {
    /// Address of the wallet that performed the action.
    pub reporter: Address,
    /// Obtained outcome of action.
    pub label: ReportLabel,
    /// Type of the action.
    pub action: ActionType,
    /// Amount of retries that it took the wallet to finish the action.
    pub retries: usize,
    /// Duration of the latest execution attempt.
    pub time: Duration,
}

/// Builder structure for `Report`.
#[derive(Debug, Clone)]
pub struct ReportBuilder {
    report: Report,
}

impl Default for ReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportBuilder {
    pub fn new() -> Self {
        Self {
            report: Report {
                reporter: Default::default(),
                label: ReportLabel::done(),
                action: ActionType::Tx(TxActionType::Transfer),
                retries: 0,
                time: Default::default(),
            },
        }
    }

    pub fn reporter(mut self, reporter: Address) -> Self {
        self.report.reporter = reporter;
        self
    }

    pub fn label(mut self, label: ReportLabel) -> Self {
        self.report.label = label;
        self
    }

    pub fn action(mut self, action: impl Into<ActionType>) -> Self {
        self.report.action = action.into();
        self
    }

    pub fn retries(mut self, retries: usize) -> Self {
        self.report.retries = retries;
        self
    }

    pub fn time(mut self, time: Duration) -> Self {
        self.report.time = time;
        self
    }

    pub fn finish(self) -> Report {
        self.report
    }
}

/// Denotes the outcome of a performed action.
#[derive(Debug, Clone)]
pub enum ReportLabel {
    ActionDone,
    ActionSkipped { reason: String },
    ActionFailed { error: String },
}

impl ReportLabel {
    pub fn done() -> Self {
        Self::ActionDone
    }

    pub fn skipped(reason: &str) -> Self {
        Self::ActionSkipped {
            reason: reason.into(),
        }
    }

    pub fn failed(error: &str) -> Self {
        Self::ActionFailed {
            error: error.into(),
        }
    }
}

/// Denotes the type of executed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TxActionType {
    Transfer,
    Withdraw,
    ForcedExit,
    ChangePubKey,
    FullExit,
    Deposit,
}

impl All for TxActionType {
    fn all() -> &'static [Self] {
        const ALL: &[TxActionType] = &[
            TxActionType::Transfer,
            TxActionType::Withdraw,
            TxActionType::ForcedExit,
            TxActionType::ChangePubKey,
            TxActionType::FullExit,
            TxActionType::Deposit,
        ];

        ALL
    }
}

impl From<TxType> for TxActionType {
    fn from(command: TxType) -> Self {
        match command {
            TxType::Deposit => Self::Deposit,
            TxType::TransferToNew | TxType::TransferToExisting => Self::Transfer,
            TxType::WithdrawToSelf | TxType::WithdrawToOther => Self::Withdraw,
            TxType::FullExit => Self::FullExit,
            TxType::ChangePubKey => Self::ChangePubKey,
        }
    }
}

/// Denotes the type of the performed API action.
/// Currently loadtest doesn't do any API actions (only as part of the transactions flow), thus enum exists
/// for the future.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiActionType {}

impl All for ApiActionType {
    fn all() -> &'static [Self] {
        const ALL: &[ApiActionType] = &[];

        ALL
    }
}

impl From<ApiRequestCommand> for ApiActionType {
    fn from(_: ApiRequestCommand) -> Self {
        todo!()
    }
}

/// Generic wrapper of all the actions that can be done in loadtest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    Tx(TxActionType),
    Api(ApiActionType),
    Batch { batch_size: usize },
}

impl From<TxActionType> for ActionType {
    fn from(action: TxActionType) -> Self {
        Self::Tx(action)
    }
}

impl From<ApiActionType> for ActionType {
    fn from(action: ApiActionType) -> Self {
        Self::Api(action)
    }
}

impl From<Command> for ActionType {
    fn from(command: Command) -> Self {
        match command {
            Command::SingleTx(tx_command) => Self::Tx(tx_command.command_type.into()),
            Command::Batch(tx_commands) => Self::Batch {
                batch_size: tx_commands.len(),
            },
            Command::ApiRequest(api_request) => Self::Api(api_request.into()),
        }
    }
}

impl ActionType {
    /// Returns the vector containing the list of all the supported actions.
    /// May be useful in different collectors to initialize their internal states.
    pub fn all() -> Vec<Self> {
        let batch_action_types =
            (1..=MAX_BATCH_SIZE).map(|batch_size| ActionType::Batch { batch_size });

        TxActionType::all()
            .iter()
            .copied()
            .map(Self::from)
            .chain(ApiActionType::all().iter().copied().map(Self::from))
            .chain(batch_action_types)
            .collect()
    }
}
