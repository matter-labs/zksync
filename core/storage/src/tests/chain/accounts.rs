// External imports
// Workspace imports
use models::node::AccountMap;
use models::Action;
// Local imports
use super::{block::apply_random_updates, utils::get_operation};
use crate::tests::{create_rng, db_test};
use crate::{
    chain::{account::AccountSchema, block::BlockSchema},
    prover::ProverSchema,
    StorageProcessor,
};

/// Checks that stored accounts can be obtained once they're committed.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn stored_accounts() {
    let _ = env_logger::try_init();
    let mut rng = create_rng();

    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Create several accounts.
        let (accounts_block, updates_block) = apply_random_updates(AccountMap::default(), &mut rng);

        // Execute and commit block with them.
        BlockSchema(&conn).execute_operation(get_operation(1, Action::Commit, updates_block))?;

        // Get the accounts by their addresses.
        for (account_id, account) in accounts_block.iter() {
            let mut account = account.clone();
            let account_state = AccountSchema(&conn).account_state_by_address(&account.address)?;

            // Check that committed state is available, but verified is not.
            assert!(
                account_state.committed.is_some(),
                "No committed state for account"
            );
            assert!(
                account_state.verified.is_none(),
                "Block is not verified, but account has a verified state"
            );

            // Compare the obtained stored account with expected one.
            let (got_account_id, got_account) = account_state.committed.unwrap();

            // We have to copy this field, since it is not initialized by default.
            account.pub_key_hash = got_account.pub_key_hash.clone();

            assert_eq!(got_account_id, *account_id);
            assert_eq!(got_account, account);

            // Also check `last_committed_state_for_account` method.
            assert_eq!(
                AccountSchema(&conn).last_committed_state_for_account(*account_id)?,
                Some(got_account)
            );
        }

        // Now add a proof, verify block and apply a state update.
        ProverSchema(&conn).store_proof(1, &Default::default())?;
        BlockSchema(&conn).execute_operation(get_operation(
            1,
            Action::Verify {
                proof: Default::default(),
            },
            Vec::new(),
        ))?;

        // After that all the accounts should have a verified state.
        for (account_id, account) in accounts_block {
            let account_state = AccountSchema(&conn).account_state_by_address(&account.address)?;

            assert!(
                account_state.committed.is_some(),
                "No committed state for account"
            );
            assert!(
                account_state.verified.is_some(),
                "No verified state for the account"
            );

            // Compare the obtained stored account with expected one.
            let (got_account_id, got_account) = account_state.verified.unwrap();

            assert_eq!(got_account_id, account_id);
            assert_eq!(got_account, account);

            // Also check `last_verified_state_for_account` method.
            assert_eq!(
                AccountSchema(&conn).last_verified_state_for_account(account_id)?,
                Some(got_account)
            );
        }

        Ok(())
    });
}
