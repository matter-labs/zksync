### zkSync

- Server: Robustness of the fee ticker's API interacting module was increased.
- Server: A possibility to get an Ethereum tx hash for withdrawal operation was added.
- Prover: Bug with delay between receiving a job and starting sending heartbeats was fixed.
- Server: Blocks that contain withdraw operations are sealed faster.
- Server: Added support for non-standard Ethereum signatures.
- Server: `eth_sender` module now can be disabled. 
- Server: Transfer to zero address (0x00..00) is now forbidden in zkSync.
- Server: WebSocket server now uses more threads for handling incoming requests.
