use std::time::Duration;

use zksync_types::Address;

use crate::command::TxType;

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

#[derive(Debug, Clone, Copy)]
pub enum TxActionType {
    Transfer,
    Withdraw,
    ForcedExit,
    ChangePubKey,
    FullExit,
    Deposit,
}

impl From<TxType> for TxActionType {
    fn from(command: TxType) -> Self {
        match command {
            TxType::Deposit => Self::Deposit,
            TxType::TransferToNew | TxType::TransferToExisting => Self::Transfer,
            TxType::WithdrawToSelf | TxType::WithdrawToOther => Self::Withdraw,
            TxType::FullExitToSelf | TxType::FullExitToOther => Self::FullExit,
            TxType::ChangePubKey => Self::ChangePubKey,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ApiActionType {}

#[derive(Debug, Clone, Copy)]
pub enum ActionType {
    Tx(TxActionType),
    Api(ApiActionType),
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

#[derive(Debug, Clone)]
pub struct Report {
    pub reporter: Address,
    pub label: ReportLabel,
    pub action: ActionType,
    pub retries: usize,
    pub time: Duration,
}
