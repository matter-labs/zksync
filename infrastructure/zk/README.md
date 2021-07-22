# `zk`

### Scope

This document describes how to extend the `zk` tool. For usage tips please use `--help` flag available on all
subcommands.

### Adding a top-level subcommand

To add a top-level subcommand `cmd` follow these steps:

- create a file `src/cmd.ts`
- implement all needed functionality (preferably export it, too)
- create an `export const command` via [`commander.js`](https://github.com/tj/commander.js) API, possibly extending
  itself with subcommands
- declare `import { command as cmd } from './cmd';` in `src/index.ts`
- add `cmd` as a subcommand via `.addCommand(cmd)`
- notify the team to rebuild `zk` upon merge

If `cmd` will have deeply nested subcommands, consider creating a directory `cmd/` instead of a file. See `db/`
structure as an example.

### Extending an existing subcommand

Simply add the needed functionality to the corresponding `.ts` file and add your subcommand to the existing
`const command` via `.command(...)` API. Don't forget to notify the team to rebuild `zk` upon merge.
