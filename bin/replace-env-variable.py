#!/usr/bin/python3

import fileinput
import sys

def process_line(line, env_name, env_val):
    line_env_name = line.split("=")[0]
    if line_env_name == env_name:
        print("%s=%s" % (env_name, env_val))
    else:
        print(line, end = '')

if __name__ == "__main__":
    file = sys.argv[1]
    env = sys.argv[2].split("=")
    env_name = env[0]
    env_val = env[1]
    for line in fileinput.input(file, inplace=True):
        process_line(line, env_name, env_val)
