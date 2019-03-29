# 0.3.5 -- new eth filter ID changes

1. Adds padded quantities
2. Fixed problem where number ID 1 for filter ID encodes to 0x1, when it should be 0x01 (with padding)
3. Methods affected: `eth.getFilterChanges` `eth.uninstallFilter` `eth.getFilterLogs`

# 0.3.4 -- added new ethjs-format

1. Unhandled promise rejection fixed, and is no longer being swolloed.
2. ethjs-rpc bump to 0.1.9

# 0.2.6 -- added new ethjs-format

1. no longer padds quantity hex values, as per standard.

# 0.2.4 -- personal sign and ecrecover

# 0.2.3 -- package updates

1. Update ethjs-rpc, handle 405 errors better

# 0.2.1 -- handle non RPC errors better

1. Handle non rpc errors better

# 0.2.0 -- handle 500 errors better

1. Handles 500/404/303 errors

# 0.1.8 -- bn formatting update

1. Bignumber formatting update

# 0.1.7 -- Better RPC error handling

1. Better RPC error handling

# 0.1.6 -- Strinigy RPC error

1. Added JSON.strinify for RPC error handling

# 0.1.5 -- format update

1. Tigher formatting enforcement
2. Small schema update

# 0.1.4 -- less dependencies

1. Better formatting
2. Less dependencies
3. ID generation done in house
4. 25kb less file size
5. More docs

# 0.1.2 -- config fixes

1. webpack config updates
2. build config updates

# 0.1.1 -- new packages

1. new ethjs-format
2. more docs

# 0.0.5 -- refactor

1. code cleanup
2. more coverage
3. better error handling
4. less dependencies

# 0.0.4 -- promises, louder errors, more tests

1. added promises
2. louder errors
3. more test coverage

# 0.0.3 -- options with debug logging and other features

1. added low level complete logging `new Eth(provider, { debug: false, logger: console, jsonSpace: 0 })`
2. more tests

# 0.0.2 -- handle eth_getFilterChanges during Block and Pending Tx filter

1. handle getFilterChanges during BlockFilter and PendingTxFilter.

# 0.0.1 -- ethjs-query

1. Basic testing
2. Basic docs
3. License
4. linting
5. basic exports
