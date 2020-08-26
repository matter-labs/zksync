use anyhow::{Context, Result};
use std::process::Command;
use web3::types::Address;

pub fn run_external_command(command: &str, args: &[&str]) -> Result<String> {
    let result = Command::new(command)
        .args(args)
        .output()
        .context(format!("failed to execute command: {}", command))?;

    let stdout = String::from_utf8(result.stdout).context("stdout is not valid utf8")?;
    let stderr = String::from_utf8(result.stderr).context("stderr is not valid utf8")?;

    if !result.status.success() {
        return Err(anyhow::anyhow!(
            "failed to run exetrnal command {}:\nstdout: {}\nstderr: {}",
            command,
            stdout,
            stderr
        ));
    }
    Ok(stdout)
}

pub fn str_to_address(value: &str) -> Result<Address> {
    let str_addr = value["0x".len()..].parse().context("Error parse address")?;
    Ok(str_addr)
}

pub fn get_matches_from_lines(stream: &str, pattern: &str) -> Result<String> {
    let lines = stream.split_whitespace().collect::<Vec<_>>();

    for std_out_line in lines {
        if std_out_line.starts_with(pattern) {
            return Ok(std_out_line.to_string());
        }
    }
    Err(anyhow::anyhow!(
        "error of finding the pattern '{}' in stream",
        pattern
    ))
}
