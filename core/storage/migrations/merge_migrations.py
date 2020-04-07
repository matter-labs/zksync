#!/usr/bin/env python3

"""
This script merges all the existing diesel migrations into one big migration.
It is pretty dumb, since it only goes through the existing migrations and create
two big files: "up.sql" and "down.sql" with contents of all the migrations that
exist in this folder. These merged migrations are required to be optimized manually
after merging (e.g. if some table was created and later removed, you'll have to
remove the `CREATE TABLE` and `DROP TABLE` statements in `up.sql` and do the same
for the statements in the `down.sql`).

Note that this script must not be run and should be removed once ZKSync node will
be run in the production.
"""

from typing import List
import os
import sys

def get_migrations_folders() -> List[str]:
    """Parses the folder contents, loading the list of folders containing migrations."""

    initial_setup_folder = "00000000000000_diesel_initial_setup"

    # Load the current directory contents and retain the folders only.
    folders = list(filter(os.path.isdir, os.listdir(".")))
    # Remove the default diesel migration.
    folders.remove(initial_setup_folder)

    return sorted(folders)

def merge_migrations() -> None:
    """Merges the migrations into two filed: `up.sql` and `down.sql`"""
    folders = get_migrations_folders()
    current_dir = os.getcwd()

    up_sql_contents = ""
    down_sql_contents = ""

    # Go through every folder and load the contents of the `up.sql`/`down.sql` files.
    for folder in folders:
        dir_path = os.path.join(current_dir, folder)

        up_sql_path = os.path.join(dir_path, "up.sql")
        down_sql_path = os.path.join(dir_path, "down.sql")

        with open(up_sql_path, "r") as up_file:
            up_sql_contents += up_file.read()

        with open(down_sql_path, "r") as down_file:
            down_sql_contents += down_file.read()

    # Store merged files.
    with open("up.sql", "w") as up_out:
        up_out.write(up_sql_contents)

    with open("down.sql", "w") as down_out:
        down_out.write(down_sql_contents)

def ensure_folder() -> None:
    """Verifies that script is launched from the `migrations` folder."""
    current_dir_full = os.getcwd()
    current_dir = os.path.basename(current_dir_full)

    if current_dir != "migrations":
        print(f"Script must be launched from the `storage/migrations` folder, \
                but current directory is {current_dir_full}")
        sys.exit(1)

if __name__ == "__main__":
    ensure_folder()
    merge_migrations()
