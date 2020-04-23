//! Leader election is a always live routine that continuously votes to become the leader.

use models::node::config::LEADER_LOOKUP_INTERVAL;
use std::thread;

/// Places itself as candidate to leader_election table and continuously looks up who is current leader.
/// Lookups happen with `LEADER_LOOKUP_INTERVAL` period.
/// Current leader is the oldest candidate in leader_election table who did not bail.
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
        .map_err(|e| failure::format_err!("could not access storage: {}", e))?;
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
        thread::sleep(LEADER_LOOKUP_INTERVAL);
    }
    Ok(())
}
