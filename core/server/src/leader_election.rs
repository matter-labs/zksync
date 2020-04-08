//! Leader election is a always live routine that continuously votes to become the leader.

use models::node::config::LEADER_ELECTION_ITERVAL;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Continuously votes to be the leader and exits when it becomes the leader.
/// Voting happens with `LEADER_ELECTION_ITERVAL`.
/// The leader retirement handled by external service.
///
/// # Panics
///
/// Panics on failed conenction to db.
pub fn vote_to_be_the_leader(
    name: String,
    connection_pool: storage::ConnectionPool,
) -> Result<(), failure::Error> {
    let st = connection_pool
        .access_storage()
        .map_err(|e| failure::format_err!("could not to access store: {}", e))?;
    loop {
        let won_election = st
            .leader_election_schema()
            .vote(&name)
            .map_err(|e| failure::format_err!("could not to vote for leader: {}", e))?;
        if won_election {
            break;
        }
        thread::sleep(LEADER_ELECTION_ITERVAL);
    }
    Ok(())
}
