//! Leader election is a always live routine that continuously votes to become the leader.

use std::time::Instant;
use std::thread;
use std::sync::mpsc;

/// run_leader_election_task continuously votes to be the leader and notifies over channel when becomes the leader.
/// Voting happens with `LEADER_ELECTION_ITERVAL`.
/// The curent leader looses its position if vote hasn't been updated for `LEADER_ELECTION_TIMEOUT`.
/// 
/// # Panics
/// 
/// If current replica was the leader and it looses its position, thread panics.
/// This is done due to assumption that the leader position was taken over by now
/// and current replica needs to stop all conflicting routines and
/// be restarted in observer voting mode. This edge case is not expected to happen offen.
pub fn run_leader_election_task(
    name: String,
    connection_pool: storage::ConnectionPool,
    election_tx: mpsc::Sender<()>,
) {
    let mut last_elected: Option<Instant> = None;
    loop {
        if let Some(t) = last_elected {
            // Panic is the leader position was taken over by this time.
            if t.elapsed() >= LEADER_ELECTION_TIMEOUT {
                let msg = "LEADER lost its elected position: timeout";
                error!("{}", msg);
                panic!("{}", msg);
            }
        }
        thread::sleep(LEADER_ELECTION_ITERVAL);
        let st = connection_pool.access_storage().map_err(|e| error!("failed to access store: {}", e)).unwrap();
        let elected_as_leader = st.leader_election_schema()
                .vote_for_leader(&name, LEADER_ELECTION_TIMEOUT)
                .map_err(|e| error!("failed to vote for leader: {}", e))
                .unwrap();
        if elected_as_leader {
            // Notify of becoming leader.
            // Notify only once.
            if last_elected == None {
                election_tx.send(());
            }
            last_elected = Some(Instant::now());
        }
    }
}
