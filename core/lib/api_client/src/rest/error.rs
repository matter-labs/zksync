// Built-in uses
use std::fmt::{self, Display};

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses

// Local uses

/// The error body that is returned in the response content.
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// A URI reference that identifies the problem type.
    #[serde(rename = "type", default, skip_serializing_if = "String::is_empty")]
    pub docs_uri: String,
    /// A short, human-readable summary of the problem.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// A human-readable explanation specific to this occurrence of the problem.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Error location in the source code.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    /// Internal error code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<u64>,
}

impl Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.title)
    }
}
