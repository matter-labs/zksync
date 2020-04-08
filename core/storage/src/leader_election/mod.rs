// Built-in deps
// External imports
use diesel::prelude::*;
use diesel::result::Error as DieselError;
// Local imports
use crate::schema::leader_election;
use crate::schema::leader_election::dsl;
use crate::StorageProcessor;

pub mod records;

/// Schema for election for a leader position and getting current leader.
#[derive(Debug)]
pub struct LeaderElectionSchema<'a>(pub &'a StorageProcessor);

impl<'a> LeaderElectionSchema<'a> {
    // Tries to take leader position if empty.
    pub fn vote(&self, name: &str) -> QueryResult<bool> {
        let name = name.to_owned();
        self.0.conn().transaction::<bool, DieselError, _>(move || {
            if let Some(leader_name) = self.leader()? {
                Ok(leader_name == name)
            } else {
                diesel::insert_into(leader_election::table)
                    .values(&records::NewLeaderElection {
                        name,
                        retired_at: None,
                    })
                    .execute(self.0.conn())?;
                Ok(true)
            }
        })
    }

    // Returns current leader name.
    pub fn leader(&self) -> QueryResult<Option<String>> {
        if let Some(election) = self.active_election()? {
            Ok(Some(election.name))
        } else {
            Ok(None)
        }
    }

    fn active_election(&self) -> QueryResult<Option<records::LeaderElection>> {
        leader_election::table
            .filter(dsl::retired_at.is_null())
            .first(self.0.conn())
            .optional()
    }

    pub fn retire(&self) -> QueryResult<()> {
        diesel::update(leader_election::table.filter(dsl::retired_at.is_null()))
            .set(dsl::retired_at.eq(diesel::dsl::now))
            .execute(self.0.conn())?;
        Ok(())
    }
}
