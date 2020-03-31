//! Observer mode continuously checks the database and keeps updated state of the accounts in memory.
//! The state is then fed to other actors when server transitions to the leader mode.


struct Observer {
    state: Option<(u32, AccountMap)>,
}
