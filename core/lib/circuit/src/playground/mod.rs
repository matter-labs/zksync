//! this module is created for fast prototyping and experimentation,
#![allow(clippy::all)]

use std::path::PathBuf;

/// handy function to get file path in file_dump dir
pub fn get_path_in_file_dump_dir(filename: &str) -> PathBuf {
    let mut base_dir = std::env::var("ZKSYNC_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Current dir not set"));
    base_dir.push("core");
    base_dir.push("circuit");
    base_dir.push("src");
    base_dir.push("playground");
    base_dir.push("file_dump");
    base_dir.push(filename);
    base_dir
}

pub mod plonk_playground;
