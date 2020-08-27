#!/usr/bin/env node

import { program } from 'commander';

program
    .version("0.1.0")
    .option("-t, --test", "test option")
    .parse(process.argv);

if (program.test) {
    console.log("Hello from zcli");
}
