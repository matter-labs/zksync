//! Leader election is a always live routine that continuously votes to become the leader.

use std::time::Instant;
use std::thread;
use std::sync::mpsc;
use models::node::config::LEADER_ELECTION_ITERVAL;

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
    let mut won_election = false;
    for !won_election {
        thread::sleep(LEADER_ELECTION_ITERVAL);
        let st = connection_pool.access_storage().map_err(|e| failure::format_err!("could not to access store: {}", e))?;
        won_election = st.leader_election_schema()
                .vote(&name)
                .map_err(|e| failure::format_err!("could not to vote for leader: {}", e))?;
    }
}
