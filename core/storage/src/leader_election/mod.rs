// Built-in deps
use std::time;
// External imports
use chrono::prelude::*;
use diesel::result::Error as DieselError;
use diesel::prelude::*;
use diesel::dsl::{insert_into, update};
// Workspace imports
// Local imports
use crate::StorageProcessor;
use crate::schema::*;

pub mod records;

/// Schema for election for a leader position and getting current leader.
#[derive(Debug)]
pub struct LeaderElectionSchema<'a>(pub &'a StorageProcessor);

impl<'a> LeaderElectionSchema<'a> {
    // Sets `name` as leader for next `election_timeout` if leader position is not occupied by someone else.
    pub fn vote_for_leader(&self, name: &str, election_timeout: time::Duration) -> QueryResult<()> {
        let name = name.to_owned();
        self.0.conn().transaction::<_, DieselError, _>(move || {
            let last_election = self.last_election()?;
            let now = Utc::now();
            let new_election = records::LeaderElection{
                id: true,
                name,
                voted_at: NaiveDateTime::from_timestamp(now.timestamp(), 0),
            };
            if let Some(last_election) = last_election {
                let since_last_election = time::Duration::from_millis((now.timestamp_millis() - last_election.voted_at.timestamp_millis()) as u64);
                let last_election_timed_out = since_last_election > election_timeout;
                if last_election.name == new_election.name || last_election_timed_out {
                    return update(leader_election::table.filter(leader_election::id.eq(true)))
                        .set((leader_election::voted_at.eq(new_election.voted_at), leader_election::name.eq(new_election.name)))
                        .execute(self.0.conn())
                        .map(drop);
                }
                Ok(())
            } else {
                insert_into(leader_election::table)
                    .values(&new_election)
                    .execute(self.0.conn())
                    .map(drop)
            }
        })?;
        Ok(())
    }

    // Returns last voted leader if any.
    pub fn last_leader(&self) -> QueryResult<Option<String>> {
        if let Some(last) = self.last_election()? {
            Ok(Some(last.name))
        } else {
            Ok(None)
        }
    }

    fn last_election(&self) -> QueryResult<Option<records::LeaderElection>> {
        let elections: Vec<records::LeaderElection> = leader_election::table
                .find(true)
                .load(self.0.conn())?;
        if elections.len() == 0 {
            Ok(None)
        } else {
            Ok(Some(elections[0].clone()))
        }
    }
}
