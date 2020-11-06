//! The module is responsible for saving the results of load testing in the file system.

// Built-in import
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};
// External uses
use chrono::{SecondsFormat, Utc};
use futures::prelude::*;
use once_cell::sync::OnceCell;
use tokio::{
    fs::File,
    prelude::*,
    sync::mpsc::{channel, Receiver, Sender},
};
// Workspace uses
// Local uses
use crate::{config::AccountInfo, executor::Report};

struct Session {
    sender: Sender<Message>,
    out_dir: PathBuf,
}

static SESSION: OnceCell<Session> = OnceCell::new();

fn session() -> &'static Session {
    SESSION
        .get()
        .expect("An attempt to use unitialized session")
}

#[derive(Debug)]
enum Message {
    WalletCreated(AccountInfo),
    ErrorOccurred { category: String, reason: String },
}

/// Creates a new load test session in the specified directory.
///
/// It creates a such files:
///
/// - `error.log` which is contains errors occurred during the load test execution.
///
/// - `wallets.log` which is contains keypairs of intermediate wallets to make
///   it possible to refund if load test fails.
///
/// - `output.json` which is contains a full summary of the load testing session.
pub async fn init_session(out_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let (sender, receiver) = channel(2048);
    let out_dir = out_dir.as_ref().to_owned();

    log::info!(
        "Load tests log will saved into the file://{} directory.",
        out_dir.to_string_lossy()
    );

    tokio::spawn(run_messages_writer(out_dir.clone(), receiver));
    SESSION
        .set(Session { sender, out_dir })
        .map_err(|_| anyhow::anyhow!("Unable to establish load test session"))
}

pub async fn finish_session(report: &Report) -> anyhow::Result<()> {
    let out_dir = &session().out_dir;

    let mut output = File::create(out_dir.join("output.json")).await?;
    let json = serde_json::to_string_pretty(report)?;
    output.write_all(&json.as_bytes()).await?;
    output.shutdown();

    Ok(())
}

/// Saves specified wallet in the file log.
pub fn save_wallet(info: AccountInfo) {
    let msg = Message::WalletCreated(info);

    tokio::spawn(async move {
        session()
            .sender
            .clone()
            .send(msg)
            .await
            .expect("Unable to save wallet")
    });
}

/// Saves specified error message in the file log.
pub fn save_error(category: &str, reason: impl Display) {
    let msg = Message::ErrorOccurred {
        category: category.to_string(),
        reason: reason.to_string(),
    };

    tokio::spawn(async move {
        session()
            .sender
            .clone()
            .send(msg)
            .await
            .expect("Unable to save error message")
    });
}

async fn run_messages_writer(
    out_dir: PathBuf,
    mut receiver: Receiver<Message>,
) -> anyhow::Result<()> {
    let mut error_log = File::create(out_dir.join("error.log")).await?;
    let mut wallets = File::create(out_dir.join("wallets.log")).await?;

    while let Some(msg) = receiver.next().await {
        match msg {
            Message::WalletCreated(wallet) => {
                let mut json = serde_json::to_vec_pretty(&wallet)?;
                json.extend_from_slice(b",\n");
                wallets.write_all(&json).await?;
            }
            Message::ErrorOccurred { category, reason } => {
                let entry = format!(
                    "[{} {}] {}\n",
                    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false),
                    category,
                    reason
                );
                error_log.write_all(entry.as_bytes()).await?;
            }
        }
    }

    Ok(())
}
