# 0.2.7 -- forced support quantity padding

- QP   0 => 0x00
- Q    0 => 0x0

# 0.2.0 -- no longer padds quantity values

1. Not sure why we ever did, maybe just logical assumptions...

  - Quantity "0" => "0x0"
             "1" => "0x1" etc.. no padding of hex quantity values in the format layer.

# 0.1.8 -- added personal sign and recover

# 0.1.7 -- hex prefix

1. Updates number-to-bn

# 0.1.5 -- fixed block formatting

1. Spec down enforcement (not value up)
2. Tighter spec enforement
3. More tests on Block data structure
4. Schema update

# 0.1.4 -- removed negative number

1. Remove possibility of negative numbers on chain

# 0.1.3 -- less deps

1. New util with less dependencies
2. webpack config updates
3. build config updates

# 0.1.2 -- less dependencies

1. Removal of utf8 dependency
2. ethjs-util update
3. package config update

# 0.1.1 -- removal of BigNumber for BN

1. removal of BigNumber for BN
2. more coverage
3. more docs
4. package fixes, removals

# 0.1.0 -- better coverage

1. better coverage testing

# 0.0.9 -- more schema details for adding

1. Added additional data for the "latest" tag, flagged as:
  [0] inputs
  [1] outputs
  [2] minimum required input array length
  [3] if === 2 ? `latest` : ``

# 0.0.8 -- expose schema in exports

1. Expose entire schema in exports for other modules to use
2. More code comments

# 0.0.7 -- more schema updates

1. Define requirements further for length of calls like ssh_post etc.

# 0.0.6 -- handle BlockFilter and PendingTransactionFilter

1. Handle the bad design caveit of tx or FilterChange result

# 0.0.5 -- minor fix on eth_getCode

1. Minor fix on eth_getCode, requires 1 not 2 param length

# 0.0.4 -- minor fix on eth_txCount..

1. Minor fix on eth_getTransactionCount, required 2 instead of 1..

# 0.0.3 -- enforce input param requirements

1. Enforce input param requirements
2. Ethjs-util integration

# 0.0.2 -- Handle floats with error, switch all bn to BigNumber

1. Handle quantity floats with error (no floats on chain)
2. Switched all bignumbers from `bn.js` to `bignumber.js`
3. Enfore 20 and 32 byte lengths where required, throw if not alphanumeric

# 0.0.1 -- ethjs-formmat

1. Basic testing
2. Basic docs
3. License
