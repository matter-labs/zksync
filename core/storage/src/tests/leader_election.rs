// Built-in
use std::time::Duration;
use std::thread;
// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::StorageProcessor;

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn vote_for_leader() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        let leader_period = Duration::from_secs(1);
        let v = conn.leader_election_schema().vote_for_leader("foo", leader_period).unwrap();
        assert_eq!(v, true);
        let v = conn.leader_election_schema().vote_for_leader("bar", leader_period).unwrap();
        assert_eq!(v, false);
        assert_eq!(conn.leader_election_schema().last_leader().unwrap(), Some("foo".to_owned()));
        thread::sleep(2 * leader_period); // 2x just to be sure
        let v = conn.leader_election_schema().vote_for_leader("bar", leader_period).unwrap();
        assert_eq!(v, true);
        assert_eq!(conn.leader_election_schema().last_leader().unwrap(), Some("bar".to_owned()));
        Ok(())
    });
}
