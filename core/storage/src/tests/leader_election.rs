// Built-in
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
        let v = conn.leader_election_schema().vote("foo").unwrap();
        assert_eq!(v, true);
        let v = conn.leader_election_schema().vote("bar").unwrap();
        assert_eq!(v, false);
        assert_eq!(
            conn.leader_election_schema().leader().unwrap(),
            Some("foo".to_owned())
        );
        conn.leader_election_schema().retire().unwrap();
        let v = conn.leader_election_schema().vote("bar").unwrap();
        assert_eq!(v, true);
        assert_eq!(
            conn.leader_election_schema().leader().unwrap(),
            Some("bar".to_owned())
        );
        Ok(())
    });
}
