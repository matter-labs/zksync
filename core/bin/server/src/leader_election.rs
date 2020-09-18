//! Leader election is a always live routine that continuously votes to become the leader.

use models::node::config::LEADER_LOOKUP_INTERVAL;
use std::path::Path;
use std::process::Command;

/// Blocks thread until node is leader .
///
/// # Panics
///
/// Panics on failed connection to db.
pub fn block_until_leader() -> Result<(), failure::Error> {
    if Path::new("/etc/podinfo/labels").exists() {
        info!("Kubernetes detected, checking if node is leader");
        loop {
            let result = Command::new("kube-is-leader.sh")
                .output()
                .expect("Failed to check if node is leader");
            if result.status.success() {
                break;
            }
            std::thread::sleep(LEADER_LOOKUP_INTERVAL);
        }
    } else {
        info!("No kubernetes detected, node is selected as leader")
    }
    info!("Node is selected as leader");
    Ok(())
}
