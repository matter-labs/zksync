// Built-in deps
// External imports
use diesel::prelude::*;
use diesel::result::Error as DieselError;
// Local imports
use crate::schema::leader_election;
use crate::schema::leader_election::dsl;
use crate::StorageProcessor;
use chrono::{NaiveDateTime, Utc};

pub mod records;

/// Schema for election for a leader position and getting current leader.
#[derive(Debug)]
pub struct LeaderElectionSchema<'a>(pub &'a StorageProcessor);

impl<'a> LeaderElectionSchema<'a> {
    // Inserts a new candidate to the leader election table to become leader if all who was before bail.
    pub fn place_candidate(&self, name: &str) -> QueryResult<()> {
        let name = name.to_owned();
        diesel::insert_into(leader_election::table)
            .values(&records::NewLeaderElection { name })
            .execute(self.0.conn())?;
        Ok(())
    }

    // Returns current leader name.
    pub fn current_leader(&self) -> QueryResult<Option<records::LeaderElection>> {
        self.next_candidate()
    }

    fn next_candidate(&self) -> QueryResult<Option<records::LeaderElection>> {
        leader_election::table
            .filter(dsl::bail_at.is_null())
            .order_by(dsl::created_at.asc())
            .first(self.0.conn())
            .optional()
    }

    pub fn bail(&self, name: &str, until: Option<NaiveDateTime>) -> QueryResult<()> {
        let datetime = if let Some(datetime) = until {
            datetime
        } else {
            NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0)
        };
        let query = leader_election::table
            .filter(dsl::bail_at.is_null())
            .filter(dsl::name.eq(name.to_owned()))
            .filter(dsl::created_at.le(datetime));
        diesel::update(query)
            .set(dsl::bail_at.eq(diesel::dsl::now))
            .execute(self.0.conn())?;
        Ok(())
    }
}
