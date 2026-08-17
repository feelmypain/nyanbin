use redis::aio::ConnectionManager;
use std::{future::Future, time::Duration};
use tokio::time::timeout;

use crate::note::{Lifecycle, StoredInfo};

const RESERVE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) ~= 0 or redis.call('EXISTS', KEYS[2]) ~= 0 then
  return {'collision'}
end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
local expires_at = string.format('%.0f', now_ms + tonumber(ARGV[1]) * 1000)
redis.call('HSET', KEYS[1], 'expires_at', expires_at, 'max_reads', ARGV[2], 'delete_hash', ARGV[3])
redis.call('EXPIRE', KEYS[1], ARGV[4])
return {'ok', expires_at}
"#;

const COMMIT_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return 'missing' end
if redis.call('EXISTS', KEYS[2]) ~= 0 then return 'collision' end
local values = redis.call('HMGET', KEYS[1], 'expires_at', 'max_reads', 'delete_hash')
if values[1] ~= ARGV[1] or values[2] ~= ARGV[2] or values[3] ~= ARGV[3] then return 'mismatch' end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(ARGV[1]) <= now_ms then
  redis.call('DEL', KEYS[1])
  return 'missing'
end
redis.call('HSET', KEYS[2], 'protocol', '1', 'envelope', ARGV[4], 'expires_at', ARGV[1], 'max_reads', ARGV[2], 'remaining_reads', ARGV[2], 'delete_hash', ARGV[3], 'password_protected', ARGV[5])
redis.call('PEXPIREAT', KEYS[2], ARGV[1])
redis.call('DEL', KEYS[1])
return 'ok'
"#;

const INFO_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return {'missing'} end
local v = redis.call('HMGET', KEYS[1], 'protocol', 'expires_at', 'max_reads', 'remaining_reads', 'password_protected')
if v[1] ~= '1' or not v[2] or v[3] == false or v[4] == false then return {'corrupt'} end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(v[2]) <= now_ms then redis.call('DEL', KEYS[1]); return {'missing'} end
return {'ok', v[2], v[3], v[4], v[5] or '0'}
"#;

const REVEAL_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return {'missing'} end
local v = redis.call('HMGET', KEYS[1], 'protocol', 'envelope', 'expires_at', 'max_reads', 'remaining_reads')
if v[1] ~= '1' or not v[2] or not v[3] or v[4] == false or v[5] == false then return {'corrupt'} end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(v[3]) <= now_ms then redis.call('DEL', KEYS[1]); return {'missing'} end
if v[4] ~= '' then
  local remaining = tonumber(v[5])
  if not remaining or remaining <= 0 then redis.call('DEL', KEYS[1]); return {'missing'} end
  if remaining == 1 then redis.call('DEL', KEYS[1]) else redis.call('HINCRBY', KEYS[1], 'remaining_reads', -1) end
end
return {'ok', v[2]}
"#;

const DELETE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return 'missing' end
local stored = redis.call('HGET', KEYS[1], 'delete_hash')
if not stored or string.len(stored) ~= 64 or string.len(ARGV[1]) ~= 64 then return 'invalid' end
local different = 0
for index = 1, 64 do
  if string.byte(stored, index) ~= string.byte(ARGV[1], index) then different = 1 end
end
if different ~= 0 then return 'invalid' end
redis.call('DEL', KEYS[1])
return 'deleted'
"#;

const SHORT_CREATE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return {'missing'} end
local v = redis.call('HMGET', KEYS[1], 'expires_at', 'delete_hash', 'password_protected', 'short_code')
if not v[1] or not v[2] then return {'corrupt'} end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(v[1]) <= now_ms then redis.call('DEL', KEYS[1]); return {'missing'} end
if string.len(v[2]) ~= 64 or string.len(ARGV[1]) ~= 64 then return {'invalid'} end
local different = 0
for index = 1, 64 do
  if string.byte(v[2], index) ~= string.byte(ARGV[1], index) then different = 1 end
end
if different ~= 0 then return {'invalid'} end
if v[3] ~= '1' then return {'unprotected'} end
if v[4] then return {'exists', v[4]} end
if redis.call('EXISTS', KEYS[2]) ~= 0 then return {'collision'} end
redis.call('SET', KEYS[2], ARGV[3])
redis.call('PEXPIREAT', KEYS[2], v[1])
redis.call('HSET', KEYS[1], 'short_code', ARGV[2])
return {'ok'}
"#;

const SHORT_RESOLVE_SCRIPT: &str = r#"
local id = redis.call('GET', KEYS[1])
if not id then return {'missing'} end
local note_key = ARGV[1] .. 'note:' .. id
local v = redis.call('HMGET', note_key, 'protocol', 'expires_at')
if v[1] ~= '1' or not v[2] then redis.call('DEL', KEYS[1]); return {'missing'} end
local now = redis.call('TIME')
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(v[2]) <= now_ms then redis.call('DEL', note_key); redis.call('DEL', KEYS[1]); return {'missing'} end
return {'ok', id}
"#;

const RATE_LIMIT_SCRIPT: &str = r#"
local global_count = tonumber(redis.call('GET', KEYS[2])) or 0
if global_count >= tonumber(ARGV[3]) then return {-1, global_count} end
local address_count = redis.call('INCR', KEYS[1])
if address_count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
if address_count > tonumber(ARGV[2]) then return {address_count, -1} end
global_count = redis.call('INCR', KEYS[2])
if global_count == 1 then redis.call('EXPIRE', KEYS[2], ARGV[1]) end
return {address_count, global_count}
"#;

#[derive(Clone)]
pub struct Store {
    manager: ConnectionManager,
    prefix: String,
    timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveResult {
    Created { expires_at: u64 },
    Collision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitResult {
    Created,
    Missing,
    Mismatch,
    Collision,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortCreateResult {
    Created,
    Exists { code: String },
    Collision,
    Missing,
    Invalid,
    Unprotected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteResult {
    Deleted,
    Missing,
    Invalid,
}

impl Store {
    pub async fn connect(
        url: &str,
        prefix: String,
        command_timeout: Duration,
    ) -> Result<Self, String> {
        let client =
            redis::Client::open(url).map_err(|_| "invalid NYANBIN_REDIS_URL".to_string())?;
        let manager = timeout(command_timeout, ConnectionManager::new(client))
            .await
            .map_err(|_| "timed out connecting to Valkey".to_string())?
            .map_err(|_| "could not connect to Valkey".to_string())?;
        Ok(Self {
            manager,
            prefix,
            timeout: command_timeout,
        })
    }

    fn reservation_key(&self, id: &str) -> String {
        format!("{}reservation:{id}", self.prefix)
    }
    fn note_key(&self, id: &str) -> String {
        format!("{}note:{id}", self.prefix)
    }
    fn short_key(&self, code: &str) -> String {
        format!("{}short:{code}", self.prefix)
    }
    fn rate_key(&self, operation: &str, bucket: &str) -> String {
        format!("{}rate:{operation}:{bucket}", self.prefix)
    }

    async fn timed<T>(&self, future: impl Future<Output = redis::RedisResult<T>>) -> Result<T, ()> {
        timeout(self.timeout, future)
            .await
            .map_err(|_| ())?
            .map_err(|_| ())
    }

    pub async fn ping(&self) -> Result<(), ()> {
        let mut connection = self.manager.clone();
        let response: String = self
            .timed(redis::cmd("PING").query_async(&mut connection))
            .await?;
        if response == "PONG" { Ok(()) } else { Err(()) }
    }

    pub async fn reserve(
        &self,
        id: &str,
        expires_in: Duration,
        max_reads: Option<u32>,
        delete_hash: &str,
        ttl: Duration,
    ) -> Result<ReserveResult, ()> {
        let mut connection = self.manager.clone();
        let max_reads = max_reads.map(|v| v.to_string()).unwrap_or_default();
        let result: Vec<String> = self
            .timed(
                redis::Script::new(RESERVE_SCRIPT)
                    .key(self.reservation_key(id))
                    .key(self.note_key(id))
                    .arg(expires_in.as_secs())
                    .arg(max_reads)
                    .arg(delete_hash)
                    .arg(ttl.as_secs())
                    .invoke_async(&mut connection),
            )
            .await?;
        match result.first().map(String::as_str) {
            Some("ok") if result.len() == 2 => Ok(ReserveResult::Created {
                expires_at: result[1].parse().map_err(|_| ())?,
            }),
            Some("collision") if result.len() == 1 => Ok(ReserveResult::Collision),
            _ => Err(()),
        }
    }

    pub async fn commit(
        &self,
        id: &str,
        lifecycle: &Lifecycle,
        delete_hash: &str,
        envelope: &str,
        password_protected: bool,
    ) -> Result<CommitResult, ()> {
        let mut connection = self.manager.clone();
        let max_reads = lifecycle
            .max_reads
            .map(|v| v.to_string())
            .unwrap_or_default();
        let result: String = self
            .timed(
                redis::Script::new(COMMIT_SCRIPT)
                    .key(self.reservation_key(id))
                    .key(self.note_key(id))
                    .arg(lifecycle.expires_at)
                    .arg(max_reads)
                    .arg(delete_hash)
                    .arg(envelope)
                    .arg(if password_protected { "1" } else { "0" })
                    .invoke_async(&mut connection),
            )
            .await?;
        match result.as_str() {
            "ok" => Ok(CommitResult::Created),
            "missing" => Ok(CommitResult::Missing),
            "mismatch" => Ok(CommitResult::Mismatch),
            "collision" => Ok(CommitResult::Collision),
            _ => Err(()),
        }
    }

    pub async fn info(&self, id: &str) -> Result<Option<StoredInfo>, ()> {
        let mut connection = self.manager.clone();
        let values: Vec<String> = self
            .timed(
                redis::Script::new(INFO_SCRIPT)
                    .key(self.note_key(id))
                    .invoke_async(&mut connection),
            )
            .await?;
        match values.first().map(String::as_str) {
            Some("missing") => Ok(None),
            Some("ok") if values.len() == 5 => {
                let expires_at = values[1].parse().map_err(|_| ())?;
                let max_reads = optional_u32(&values[2])?;
                let remaining_reads = optional_u32(&values[3])?;
                let password_protected = values[4] == "1";
                Ok(Some(StoredInfo {
                    expires_at,
                    max_reads,
                    remaining_reads,
                    password_protected,
                }))
            }
            _ => Err(()),
        }
    }

    pub async fn reveal(&self, id: &str) -> Result<Option<String>, ()> {
        let mut connection = self.manager.clone();
        let values: Vec<String> = self
            .timed(
                redis::Script::new(REVEAL_SCRIPT)
                    .key(self.note_key(id))
                    .invoke_async(&mut connection),
            )
            .await?;
        match values.first().map(String::as_str) {
            Some("missing") => Ok(None),
            Some("ok") if values.len() == 2 => Ok(Some(values[1].clone())),
            _ => Err(()),
        }
    }

    pub async fn delete(&self, id: &str, candidate_hash: &str) -> Result<DeleteResult, ()> {
        let mut connection = self.manager.clone();
        let result: String = self
            .timed(
                redis::Script::new(DELETE_SCRIPT)
                    .key(self.note_key(id))
                    .arg(candidate_hash)
                    .invoke_async(&mut connection),
            )
            .await?;
        match result.as_str() {
            "deleted" => Ok(DeleteResult::Deleted),
            "missing" => Ok(DeleteResult::Missing),
            "invalid" => Ok(DeleteResult::Invalid),
            _ => Err(()),
        }
    }

    pub async fn create_short(
        &self,
        id: &str,
        candidate_hash: &str,
        code: &str,
    ) -> Result<ShortCreateResult, ()> {
        let mut connection = self.manager.clone();
        let result: Vec<String> = self
            .timed(
                redis::Script::new(SHORT_CREATE_SCRIPT)
                    .key(self.note_key(id))
                    .key(self.short_key(code))
                    .arg(candidate_hash)
                    .arg(code)
                    .arg(id)
                    .invoke_async(&mut connection),
            )
            .await?;
        match result.first().map(String::as_str) {
            Some("ok") if result.len() == 1 => Ok(ShortCreateResult::Created),
            Some("exists") if result.len() == 2 => Ok(ShortCreateResult::Exists {
                code: result[1].clone(),
            }),
            Some("collision") if result.len() == 1 => Ok(ShortCreateResult::Collision),
            Some("missing") if result.len() == 1 => Ok(ShortCreateResult::Missing),
            Some("invalid") if result.len() == 1 => Ok(ShortCreateResult::Invalid),
            Some("unprotected") if result.len() == 1 => Ok(ShortCreateResult::Unprotected),
            _ => Err(()),
        }
    }

    pub async fn resolve_short(&self, code: &str) -> Result<Option<String>, ()> {
        let mut connection = self.manager.clone();
        let values: Vec<String> = self
            .timed(
                redis::Script::new(SHORT_RESOLVE_SCRIPT)
                    .key(self.short_key(code))
                    .arg(&self.prefix)
                    .invoke_async(&mut connection),
            )
            .await?;
        match values.first().map(String::as_str) {
            Some("missing") if values.len() == 1 => Ok(None),
            Some("ok") if values.len() == 2 => Ok(Some(values[1].clone())),
            _ => Err(()),
        }
    }

    pub async fn rate_limit(
        &self,
        operation: &str,
        bucket: &str,
        window: Duration,
        address_limit: u32,
        global_limit: u32,
    ) -> Result<bool, ()> {
        let mut connection = self.manager.clone();
        let counts: Vec<i64> = self
            .timed(
                redis::Script::new(RATE_LIMIT_SCRIPT)
                    .key(self.rate_key(operation, &format!("address:{bucket}")))
                    .key(self.rate_key(operation, "global"))
                    .arg(window.as_secs())
                    .arg(address_limit)
                    .arg(global_limit)
                    .invoke_async(&mut connection),
            )
            .await?;
        match counts.as_slice() {
            [address_count, global_count] => Ok(rate_counts_allowed(
                *address_count,
                *global_count,
                address_limit,
                global_limit,
            )),
            _ => Err(()),
        }
    }
}

fn rate_counts_allowed(
    address_count: i64,
    global_count: i64,
    address_limit: u32,
    global_limit: u32,
) -> bool {
    address_count > 0
        && global_count > 0
        && address_count <= i64::from(address_limit)
        && global_count <= i64::from(global_limit)
}

fn optional_u32(value: &str) -> Result<Option<u32>, ()> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse().map(Some).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_optional_read_counts() {
        assert_eq!(optional_u32("").unwrap(), None);
        assert_eq!(optional_u32("3").unwrap(), Some(3));
        assert!(optional_u32("-1").is_err());
    }
    #[test]
    fn reveal_script_deletes_terminal_read_before_returning() {
        let delete = REVEAL_SCRIPT.find("redis.call('DEL', KEYS[1])").unwrap();
        let returned = REVEAL_SCRIPT.rfind("return {'ok', v[2]}").unwrap();
        assert!(delete < returned);
    }
    #[test]
    fn short_create_script_gates_on_password_protection() {
        let gate = SHORT_CREATE_SCRIPT
            .find("if v[3] ~= '1' then return {'unprotected'} end")
            .unwrap();
        let set = SHORT_CREATE_SCRIPT.find("redis.call('SET', KEYS[2]").unwrap();
        assert!(gate < set);
    }
    #[test]
    fn short_resolve_script_self_heals_dead_notes() {
        assert!(SHORT_RESOLVE_SCRIPT.contains("redis.call('DEL', KEYS[1])"));
        assert!(SHORT_RESOLVE_SCRIPT.contains("return {'ok', id}"));
    }
    #[test]
    fn short_key_stores_the_note_id() {
        assert!(SHORT_CREATE_SCRIPT.contains("redis.call('SET', KEYS[2], ARGV[3])"));
    }
    #[test]
    fn global_limit_bounds_rotating_address_buckets() {
        assert!(rate_counts_allowed(1, 3, 1, 3));
        assert!(!rate_counts_allowed(1, 4, 1, 3));
    }

    #[test]
    fn address_limit_still_bounds_a_single_bucket() {
        assert!(rate_counts_allowed(2, 2, 2, 100));
        assert!(!rate_counts_allowed(3, 3, 2, 100));
    }
}
