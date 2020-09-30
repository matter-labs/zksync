#!/bin/bash
# Generates exit proof for exodus mode exit
# Uses database to restore account tree state (can be restored from contract using `data_restore` commands)
# run with -h flag to see cli arguments
f cargo run --example generate_exit_proof --release -- $@
