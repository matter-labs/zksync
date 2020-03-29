// External imports
use chrono::prelude::*;
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Clone, Queryable, QueryableByName, Insertable, PartialEq)]
#[table_name = "leader_election"]
pub struct LeaderElection {
    pub id: bool,
    pub name: String,
    pub voted_at: NaiveDateTime, 
}
