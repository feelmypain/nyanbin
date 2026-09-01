use std::{env, process::ExitCode, time::Duration};

use nyanbin::{
    config::Config,
    note::validate_id,
    store::{SWITCH_NAMES, Store},
};

const DEFAULT_BLOCK_TTL_SECONDS: u64 = 3_600;
const MAX_BLOCK_TTL_SECONDS: u64 = 31_536_000;
const COUNTER_MINUTES: u64 = 5;
const COUNTERS: &[&str] = &[
    "reserve",
    "reserve_limited",
    "reserve_disabled",
    "commit",
    "commit_limited",
    "commit_disabled",
    "commit_pressure",
    "commit_quota",
    "reveal",
    "reveal_limited",
    "info",
    "info_limited",
    "delete",
    "delete_limited",
    "short_create",
    "short_create_limited",
    "short_create_disabled",
    "short_resolve",
    "short_resolve_limited",
    "short_resolve_disabled",
    "short_resolve_hit",
    "short_resolve_miss",
];
const RATE_OPERATIONS: &[&str] = &[
    "reserve",
    "commit",
    "reveal",
    "info",
    "delete",
    "short_create",
    "short_resolve",
];
const USAGE: &str = "Usage:
  nyanbin-admin stats
  nyanbin-admin switch <writes_off|writes_small_only|short_off|resolve_hardened> <on|off>
  nyanbin-admin revoke <note-id> --yes
  nyanbin-admin block <bucket> [--ttl <seconds>]
  nyanbin-admin unblock <bucket>
  nyanbin-admin resync";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Stats,
    Switch { name: String, enabled: bool },
    Revoke { id: String },
    Block { bucket: String, ttl: Duration },
    Unblock { bucket: String },
    Resync,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("nyanbin-admin: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let command = parse_args(env::args().skip(1))?;
    if command == Command::Help {
        println!("{USAGE}");
        return Ok(());
    }

    let config = Config::from_env()?;
    let store =
        Store::connect(&config.redis_url, config.redis_prefix, config.redis_timeout).await?;

    match command {
        Command::Help => unreachable!(),
        Command::Stats => print_stats(&store).await,
        Command::Switch { name, enabled } => {
            store
                .set_switch(&name, enabled)
                .await
                .map_err(|_| "Valkey command failed".to_string())?;
            println!("switch {name}={}", if enabled { "on" } else { "off" });
            Ok(())
        }
        Command::Revoke { id } => {
            let removed = store
                .revoke(&id)
                .await
                .map_err(|_| "Valkey command failed".to_string())?;
            println!(
                "revoke {id}={}",
                if removed { "deleted" } else { "not_found" }
            );
            Ok(())
        }
        Command::Block { bucket, ttl } => {
            store
                .block_bucket(&bucket, ttl)
                .await
                .map_err(|_| "Valkey command failed".to_string())?;
            println!("block {bucket}=on ttl={}", ttl.as_secs());
            Ok(())
        }
        Command::Unblock { bucket } => {
            let removed = store
                .unblock_bucket(&bucket)
                .await
                .map_err(|_| "Valkey command failed".to_string())?;
            println!(
                "block {bucket}={}",
                if removed { "off" } else { "not_found" }
            );
            Ok(())
        }
        Command::Resync => {
            let snapshot = store
                .resync_meter()
                .await
                .map_err(|_| "storage meter resync failed".to_string())?;
            println!("storage bytes={} notes={}", snapshot.bytes, snapshot.notes);
            Ok(())
        }
    }
}

async fn print_stats(store: &Store) -> Result<(), String> {
    let switches = store
        .switches()
        .await
        .map_err(|_| "could not read switches".to_string())?;
    let storage_bytes = store
        .meter_bytes()
        .await
        .map_err(|_| "could not read storage meter".to_string())?;
    let blocked_buckets = store
        .blocked_bucket_count()
        .await
        .map_err(|_| "could not count blocked buckets".to_string())?;

    println!("storage_bytes={storage_bytes}");
    println!("blocked_buckets={blocked_buckets}");
    println!("switch.writes_off={}", switches.writes_off);
    println!("switch.writes_small_only={}", switches.writes_small_only);
    println!("switch.short_off={}", switches.short_off);
    println!("switch.resolve_hardened={}", switches.resolve_hardened);
    println!("counters_last_{COUNTER_MINUTES}m:");
    for name in COUNTERS {
        let total: u64 = store
            .recent_counters(name, COUNTER_MINUTES)
            .await
            .map_err(|_| format!("could not read {name} counters"))?
            .into_iter()
            .map(|(_, count)| count)
            .sum();
        println!("  {name}={total}");
    }

    println!("top_rejected_buckets:");
    for operation in RATE_OPERATIONS {
        for (bucket, count) in store
            .top_rejected_buckets(operation, 5)
            .await
            .map_err(|_| format!("could not read {operation} rejection counters"))?
        {
            println!("  {operation} {bucket}={count}");
        }
    }
    Ok(())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args: Vec<String> = args.into_iter().collect();
    let values: Vec<&str> = args.iter().map(String::as_str).collect();
    match values.as_slice() {
        [] | ["help" | "--help" | "-h"] => Ok(Command::Help),
        ["stats"] => Ok(Command::Stats),
        ["resync"] => Ok(Command::Resync),
        ["switch", name, state] => {
            if !SWITCH_NAMES.contains(name) {
                return Err(format!("unknown switch {name}\n{USAGE}"));
            }
            let enabled = match *state {
                "on" => true,
                "off" => false,
                _ => return Err(format!("switch state must be on or off\n{USAGE}")),
            };
            Ok(Command::Switch {
                name: (*name).to_string(),
                enabled,
            })
        }
        ["revoke", id, "--yes"] => {
            if validate_id(id).is_err() {
                return Err("note ID must be 32 base62 characters".to_string());
            }
            Ok(Command::Revoke {
                id: (*id).to_string(),
            })
        }
        ["revoke", ..] => Err(format!(
            "revoke requires a valid note ID and --yes\n{USAGE}"
        )),
        ["block", bucket] => Ok(Command::Block {
            bucket: parse_bucket(bucket)?,
            ttl: Duration::from_secs(DEFAULT_BLOCK_TTL_SECONDS),
        }),
        ["block", bucket, "--ttl", seconds] => {
            let seconds: u64 = seconds
                .parse()
                .map_err(|_| "block TTL must be an integer number of seconds".to_string())?;
            if !(1..=MAX_BLOCK_TTL_SECONDS).contains(&seconds) {
                return Err(format!(
                    "block TTL must be between 1 and {MAX_BLOCK_TTL_SECONDS} seconds"
                ));
            }
            Ok(Command::Block {
                bucket: parse_bucket(bucket)?,
                ttl: Duration::from_secs(seconds),
            })
        }
        ["unblock", bucket] => Ok(Command::Unblock {
            bucket: parse_bucket(bucket)?,
        }),
        _ => Err(USAGE.to_string()),
    }
}

fn parse_bucket(value: &str) -> Result<String, String> {
    if value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value.to_string())
    } else {
        Err("bucket must be exactly 32 lowercase hexadecimal characters".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn revoke_requires_explicit_confirmation() {
        let id = "0123456789abcdefghijklmnopqrstuv";
        assert!(parse_args(args(&["revoke", id])).is_err());
        assert_eq!(
            parse_args(args(&["revoke", id, "--yes"])).unwrap(),
            Command::Revoke { id: id.to_string() }
        );
    }

    #[test]
    fn block_accepts_only_pseudonymous_bucket_format() {
        let bucket = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            parse_args(args(&["block", bucket, "--ttl", "90"])).unwrap(),
            Command::Block {
                bucket: bucket.to_string(),
                ttl: Duration::from_secs(90),
            }
        );
        assert!(parse_args(args(&["block", "127.0.0.1"])).is_err());
        assert!(parse_args(args(&["block", bucket, "--ttl", "0"])).is_err());
    }

    #[test]
    fn switch_names_and_states_are_closed_sets() {
        assert!(parse_args(args(&["switch", "writes_off", "on"])).is_ok());
        assert!(parse_args(args(&["switch", "unknown", "on"])).is_err());
        assert!(parse_args(args(&["switch", "writes_off", "maybe"])).is_err());
    }
}
