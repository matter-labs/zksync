//! Leader election is a always live routine that continuously votes to become the leader.

use models::node::config::LEADER_ELECTION_INTERVAL;
use std::thread;

/// Continuously votes to be the leader and exits when it becomes the leader.
/// Voting happens with `LEADER_ELECTION_INTERVAL`.
/// The leader retirement handled by external service.
///
/// # Panics
///
/// Panics on failed connection to db.
pub fn keep_voting_to_be_leader(
    name: String,
    connection_pool: storage::ConnectionPool,
) -> Result<(), failure::Error> {
    let st = connection_pool
        .access_storage()
        .map_err(|e| failure::format_err!("could not to access store: {}", e))?;
    st.leader_election_schema()
        .bail(&name, None)
        .map_err(|e| failure::format_err!("could not bail previous placements: {}", e))?;
    st.leader_election_schema()
        .place_candidate(&name)
        .map_err(|e| failure::format_err!("could not place candidate: {}", e))?;
    log::info!("placed candidate to leader election board {}", name);
    loop {
        let leader = st
            .leader_election_schema()
            .current_leader()
            .map_err(|e| failure::format_err!("could not get current leader: {}", e))?;
        if let Some(leader) = leader {
            if leader.name == name {
                break;
            }
        }
        thread::sleep(LEADER_ELECTION_INTERVAL);
    }
    Ok(())
}
