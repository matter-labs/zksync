/* 

In order for json to be outputted properly the following params should be added to
the psql

\t
\pset format unaligned

This command should be run in the psql to get the json of the account balances

*/

/* To get the content for the accounts file */
SELECT json_agg(t) FROM (SELECT * FROM accounts) t;

/* To get the content for the balances file */
SELECT json_agg(t) FROM (SELECT account_id, coin_id, balance::VARCHAR FROM balances) t;
