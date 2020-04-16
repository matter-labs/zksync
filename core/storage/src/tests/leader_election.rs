// Built-in
// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::StorageProcessor;

#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn leader_election() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        conn.leader_election_schema().place_candidate("foo")?;
        assert_eq!(
            conn.leader_election_schema()
                .current_leader()?
                .unwrap()
                .name,
            "foo".to_owned()
        );
        conn.leader_election_schema().place_candidate("bar")?;
        assert_eq!(
            &conn
                .leader_election_schema()
                .current_leader()?
                .unwrap()
                .name,
            "foo"
        );
        conn.leader_election_schema().bail("foo", None).unwrap();
        assert_eq!(
            &conn
                .leader_election_schema()
                .current_leader()?
                .unwrap()
                .name,
            "bar"
        );
        Ok(())
    });
}
