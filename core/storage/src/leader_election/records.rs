// External imports
use chrono::prelude::*;
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable, QueryableByName, PartialEq)]
#[table_name = "leader_election"]
pub struct LeaderElection {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub retired_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Insertable, PartialEq)]
#[table_name = "leader_election"]
pub struct NewLeaderElection {
    pub name: String,
    pub retired_at: Option<NaiveDateTime>,
}
