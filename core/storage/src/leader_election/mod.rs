// Built-in deps
// External imports
use diesel::prelude::*;
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
    /// Inserts a new candidate to the leader election table to become leader if all who was before bail.
    pub fn place_candidate(&self, name: &str) -> QueryResult<()> {
        let name = name.to_owned();
        // At this point of time the candidate must be the last to be leader within all active candidates.
        // In case of system inadequate behavior it is possible that candidate was placed to leader election table
        // and replica is restarted without bailing. To ensure correct order of leader election, candidate needs to bail all
        // previous placements before placing again.
        self.bail(&name, None)?;
        diesel::insert_into(leader_election::table)
            .values(&records::NewLeaderElection { name })
            .execute(self.0.conn())?;
        Ok(())
    }

    /// Returns current leader name.
    /// Current leader is the longest waiting candidate in leader election table.
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

    /// Bails candidate with given name. If `until` is None, assumes to bail all records, otherwise
    /// bails records created before or at the same time.
    pub fn bail(&self, name: &str, until: Option<NaiveDateTime>) -> QueryResult<()> {
        let datetime = if let Some(datetime) = until {
            datetime
        } else {
            let now = Utc::now();
            NaiveDateTime::from_timestamp(now.timestamp(), now.timestamp_subsec_nanos())
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
