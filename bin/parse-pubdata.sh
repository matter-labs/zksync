#!/bin/bash

. .setup_env

cargo run --bin parse_pub_data -- $1
