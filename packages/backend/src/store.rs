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

// KEYS: 1 reservation, 2 note, 3 meter, 4 writes_small_only switch,
//       5 meter revision
// ARGV: 1 expires_at, 2 max_reads, 3 delete_hash, 4 envelope, 5 password,
//       6 size, 7 budget, 8 soft, 9 hard, 10 prefix, 11 small, 12 bucket
//       quota, 13 client bucket
const COMMIT_SCRIPT: &str = r#"
local now = redis.call('TIME')
local minute = math.floor(tonumber(now[1]) / 60)
local function bump(name)
  local key = ARGV[10] .. 'ctr:' .. name .. ':' .. minute
  redis.call('INCR', key)
  redis.call('EXPIRE', key, 7200)
end
if redis.call('EXISTS', KEYS[1]) == 0 then return {'missing'} end
if redis.call('EXISTS', KEYS[2]) ~= 0 then return {'collision'} end
local values = redis.call('HMGET', KEYS[1], 'expires_at', 'max_reads', 'delete_hash')
if values[1] ~= ARGV[1] or values[2] ~= ARGV[2] or values[3] ~= ARGV[3] then return {'mismatch'} end
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(ARGV[1]) <= now_ms then
  redis.call('DEL', KEYS[1])
  return {'missing'}
end
local size = tonumber(ARGV[6])
local small = tonumber(ARGV[11])
if redis.call('EXISTS', KEYS[4]) ~= 0 and size > small then
  bump('commit_pressure')
  return {'pressure'}
end
local budget = tonumber(ARGV[7])
if budget > 0 then
  local meter = tonumber(redis.call('GET', KEYS[3])) or 0
  if meter >= tonumber(ARGV[9]) or meter + size > budget then
    bump('commit_pressure')
    return {'pressure'}
  end
  if meter >= tonumber(ARGV[8]) and size > small then
    bump('commit_pressure')
    return {'pressure'}
  end
end
local quota = tonumber(ARGV[12])
local bucket_key = ''
if quota > 0 and ARGV[13] ~= '' then
  local hour = math.floor(tonumber(now[1]) / 3600)
  bucket_key = ARGV[10] .. 'bytes:' .. hour .. ':' .. ARGV[13]
  local used = tonumber(redis.call('GET', bucket_key)) or 0
  if used + size > quota then
    bump('commit_quota')
    return {'bucket', tostring(3600 - (tonumber(now[1]) % 3600))}
  end
end
redis.call('HSET', KEYS[2], 'protocol', '1', 'envelope', ARGV[4], 'expires_at', ARGV[1], 'max_reads', ARGV[2], 'remaining_reads', ARGV[2], 'delete_hash', ARGV[3], 'password_protected', ARGV[5])
redis.call('PEXPIREAT', KEYS[2], ARGV[1])
redis.call('DEL', KEYS[1])
redis.call('INCRBY', KEYS[3], size)
redis.call('INCR', KEYS[5])
if bucket_key ~= '' then
  redis.call('INCRBY', bucket_key, size)
  redis.call('EXPIRE', bucket_key, 7200)
end
return {'ok'}
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

// KEYS: 1 note, 2 meter, 3 meter revision
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
  if remaining == 1 then
    redis.call('DEL', KEYS[1])
    local meter = redis.call('DECRBY', KEYS[2], math.floor(string.len(v[2]) * 3 / 4))
    if meter < 0 then redis.call('SET', KEYS[2], 0) end
    redis.call('INCR', KEYS[3])
  else
    redis.call('HINCRBY', KEYS[1], 'remaining_reads', -1)
  end
end
return {'ok', v[2]}
"#;

// KEYS: 1 note, 2 meter, 3 meter revision
const DELETE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return 'missing' end
local stored = redis.call('HGET', KEYS[1], 'delete_hash')
if not stored or string.len(stored) ~= 64 or string.len(ARGV[1]) ~= 64 then return 'invalid' end
local different = 0
for index = 1, 64 do
  if string.byte(stored, index) ~= string.byte(ARGV[1], index) then different = 1 end
end
if different ~= 0 then return 'invalid' end
local envelope = redis.call('HGET', KEYS[1], 'envelope')
redis.call('DEL', KEYS[1])
if envelope then
  local meter = redis.call('DECRBY', KEYS[2], math.floor(string.len(envelope) * 3 / 4))
  if meter < 0 then redis.call('SET', KEYS[2], 0) end
  redis.call('INCR', KEYS[3])
end
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

// KEYS: 1 short; ARGV: 1 prefix. Counts hits and misses per minute (no
// identifiers) and auto-arms the resolve_hardened tripwire on a miss surge.
const SHORT_RESOLVE_SCRIPT: &str = r#"
local now = redis.call('TIME')
local minute = math.floor(tonumber(now[1]) / 60)
local function bump(name)
  local key = ARGV[1] .. 'ctr:' .. name .. ':' .. minute
  local value = redis.call('INCR', key)
  redis.call('EXPIRE', key, 7200)
  return value
end
local function miss()
  local misses = bump('short_resolve_miss')
  local hits = tonumber(redis.call('GET', ARGV[1] .. 'ctr:short_resolve_hit:' .. minute)) or 0
  if misses >= 30 and misses > 4 * hits then
    redis.call('SET', ARGV[1] .. 'switch:resolve_hardened', '1', 'EX', 600)
  end
end
local id = redis.call('GET', KEYS[1])
if not id then miss(); return {'missing'} end
local note_key = ARGV[1] .. 'note:' .. id
local v = redis.call('HMGET', note_key, 'protocol', 'expires_at')
if v[1] ~= '1' or not v[2] then redis.call('DEL', KEYS[1]); miss(); return {'missing'} end
local now_ms = tonumber(now[1]) * 1000 + math.floor(tonumber(now[2]) / 1000)
if tonumber(v[2]) <= now_ms then redis.call('DEL', note_key); redis.call('DEL', KEYS[1]); miss(); return {'missing'} end
bump('short_resolve_hit')
return {'ok', id}
"#;

// KEYS: 1 address counter, 2 global counter, 3 block key, 4 disable switch,
//       5 hardened switch
// ARGV: 1 window, 2 address limit, 3 global limit, 4 prefix, 5 op name,
//       6 hardened global (0 = none), 7 client bucket
// Returns {1, 0} allowed; {0, retry_seconds} limited; {-2, 0} disabled.
const RATE_LIMIT_SCRIPT: &str = r#"
local now = redis.call('TIME')
local minute = math.floor(tonumber(now[1]) / 60)
local function bump(name)
  local key = ARGV[4] .. 'ctr:' .. name .. ':' .. minute
  redis.call('INCR', key)
  redis.call('EXPIRE', key, 7200)
end
bump(ARGV[5])
if redis.call('EXISTS', KEYS[4]) ~= 0 then
  bump(ARGV[5] .. '_disabled')
  return {-2, 0}
end
local function limited(counter_key, record_bucket)
  local ttl = redis.call('TTL', counter_key)
  if ttl < 0 then ttl = tonumber(ARGV[1]) end
  bump(ARGV[5] .. '_limited')
  if record_bucket then
    local hour = math.floor(tonumber(now[1]) / 3600)
    local rej = ARGV[4] .. 'rej:' .. ARGV[5] .. ':' .. hour
    redis.call('ZINCRBY', rej, 1, ARGV[7])
    redis.call('EXPIRE', rej, 7200)
    -- Operator telemetry is advisory. Bound it so address rotation cannot
    -- exhaust Valkey through the rejection path itself.
    local excess = redis.call('ZCARD', rej) - 1024
    if excess > 0 then redis.call('ZREMRANGEBYRANK', rej, 0, excess - 1) end
  end
  return {0, ttl}
end
if redis.call('EXISTS', KEYS[3]) ~= 0 then return limited(KEYS[3], true) end
local global_limit = tonumber(ARGV[3])
if tonumber(ARGV[6]) > 0 and redis.call('EXISTS', KEYS[5]) ~= 0 then
  global_limit = tonumber(ARGV[6])
end
local global_count = tonumber(redis.call('GET', KEYS[2])) or 0
if global_count >= global_limit then return limited(KEYS[2], false) end
local address_count = redis.call('INCR', KEYS[1])
if address_count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
if address_count > tonumber(ARGV[2]) then return limited(KEYS[1], true) end
global_count = redis.call('INCR', KEYS[2])
if global_count == 1 then redis.call('EXPIRE', KEYS[2], ARGV[1]) end
return {1, 0}
"#;

// KEYS: 1 note, 2 meter, 3 meter revision; ARGV: 1 prefix
const REVOKE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[1]) == 0 then return 0 end
local v = redis.call('HMGET', KEYS[1], 'envelope', 'short_code')
if v[2] then redis.call('DEL', ARGV[1] .. 'short:' .. v[2]) end
redis.call('DEL', KEYS[1])
if v[1] then
  local meter = redis.call('DECRBY', KEYS[2], math.floor(string.len(v[1]) * 3 / 4))
  if meter < 0 then redis.call('SET', KEYS[2], 0) end
  redis.call('INCR', KEYS[3])
end
return 1
"#;

// Atomically replaces the meter only when no note mutation occurred during
// the preceding scan. Expiries can only make the snapshot conservatively high.
const RESYNC_METER_SCRIPT: &str = r#"
local revision = tonumber(redis.call('GET', KEYS[2])) or 0
if revision ~= tonumber(ARGV[1]) then return 0 end
redis.call('SET', KEYS[1], ARGV[2])
return 1
"#;

pub const SWITCH_NAMES: [&str; 4] = [
    "writes_off",
    "short_off",
    "writes_small_only",
    "resolve_hardened",
];

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
    /// Storage budget brownout: the commit was rejected under pressure.
    Pressure,
    /// Per-client hourly byte quota exhausted; retry after the given seconds.
    QuotaExceeded {
        retry_after: u64,
    },
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDecision {
    Allowed,
    Limited {
        retry_after: u64,
    },
    /// The operation's kill switch is armed.
    Disabled,
}

/// Operator kill switches, stored as plain keys so `nyanbin-admin` can flip
/// them without restarting the service.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Switches {
    pub writes_off: bool,
    pub short_off: bool,
    pub writes_small_only: bool,
    pub resolve_hardened: bool,
}

/// Inputs for commit-time storage accounting; all byte values are decoded
/// envelope sizes.
#[derive(Debug, Clone)]
pub struct CommitQuota {
    pub envelope_bytes: u64,
    pub budget_bytes: u64,
    pub soft_bytes: u64,
    pub hard_bytes: u64,
    pub small_bytes: u64,
    pub bucket_quota: u64,
    /// Pseudonymous client bucket; empty disables per-client byte quotas.
    pub client_bucket: String,
}

/// One rate-limit check; `disable_switch`/`hardened_global` are None for
/// operations without a kill switch or tripwire ceiling.
#[derive(Debug, Clone, Copy)]
pub struct RateCheck<'a> {
    pub operation: &'a str,
    pub bucket: &'a str,
    pub window: Duration,
    pub address_limit: u32,
    pub global_limit: u32,
    pub disable_switch: Option<&'a str>,
    pub hardened_global: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterSnapshot {
    pub bytes: u64,
    pub notes: u64,
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
    fn meter_revision_key(&self) -> String {
        format!("{}meter:revision", self.prefix)
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
    fn meter_key(&self) -> String {
        format!("{}meter:bytes", self.prefix)
    }
    fn switch_key(&self, name: &str) -> String {
        format!("{}switch:{name}", self.prefix)
    }
    fn block_key(&self, bucket: &str) -> String {
        format!("{}block:{bucket}", self.prefix)
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
        quota: &CommitQuota,
    ) -> Result<CommitResult, ()> {
        let mut connection = self.manager.clone();
        let max_reads = lifecycle
            .max_reads
            .map(|v| v.to_string())
            .unwrap_or_default();
        let result: Vec<String> = self
            .timed(
                redis::Script::new(COMMIT_SCRIPT)
                    .key(self.reservation_key(id))
                    .key(self.note_key(id))
                    .key(self.meter_key())
                    .key(self.switch_key("writes_small_only"))
                    .key(self.meter_revision_key())
                    .arg(lifecycle.expires_at)
                    .arg(max_reads)
                    .arg(delete_hash)
                    .arg(envelope)
                    .arg(if password_protected { "1" } else { "0" })
                    .arg(quota.envelope_bytes)
                    .arg(quota.budget_bytes)
                    .arg(quota.soft_bytes)
                    .arg(quota.hard_bytes)
                    .arg(&self.prefix)
                    .arg(quota.small_bytes)
                    .arg(quota.bucket_quota)
                    .arg(&quota.client_bucket)
                    .invoke_async(&mut connection),
            )
            .await?;
        match result.first().map(String::as_str) {
            Some("ok") => Ok(CommitResult::Created),
            Some("missing") => Ok(CommitResult::Missing),
            Some("mismatch") => Ok(CommitResult::Mismatch),
            Some("collision") => Ok(CommitResult::Collision),
            Some("pressure") => Ok(CommitResult::Pressure),
            Some("bucket") if result.len() == 2 => Ok(CommitResult::QuotaExceeded {
                retry_after: result[1].parse().map_err(|_| ())?,
            }),
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
                    .key(self.meter_key())
                    .key(self.meter_revision_key())
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
                    .key(self.meter_key())
                    .key(self.meter_revision_key())
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

    pub async fn rate_limit(&self, check: RateCheck<'_>) -> Result<RateDecision, ()> {
        let mut connection = self.manager.clone();
        // A switch name that is never set, for operations without one.
        let disable_key = self.switch_key(check.disable_switch.unwrap_or("none"));
        let counts: Vec<i64> = self
            .timed(
                redis::Script::new(RATE_LIMIT_SCRIPT)
                    .key(self.rate_key(check.operation, &format!("address:{}", check.bucket)))
                    .key(self.rate_key(check.operation, "global"))
                    .key(self.block_key(check.bucket))
                    .key(disable_key)
                    .key(self.switch_key("resolve_hardened"))
                    .arg(check.window.as_secs())
                    .arg(check.address_limit)
                    .arg(check.global_limit)
                    .arg(&self.prefix)
                    .arg(check.operation)
                    .arg(check.hardened_global.unwrap_or(0))
                    .arg(check.bucket)
                    .invoke_async(&mut connection),
            )
            .await?;
        match counts.as_slice() {
            [1, _] => Ok(RateDecision::Allowed),
            [0, retry] => Ok(RateDecision::Limited {
                retry_after: u64::try_from(*retry).unwrap_or(1).max(1),
            }),
            [-2, _] => Ok(RateDecision::Disabled),
            _ => Err(()),
        }
    }

    pub async fn switches(&self) -> Result<Switches, ()> {
        let mut connection = self.manager.clone();
        let values: Vec<Option<String>> = self
            .timed(
                redis::cmd("MGET")
                    .arg(self.switch_key("writes_off"))
                    .arg(self.switch_key("short_off"))
                    .arg(self.switch_key("writes_small_only"))
                    .arg(self.switch_key("resolve_hardened"))
                    .query_async(&mut connection),
            )
            .await?;
        match values.as_slice() {
            [writes, short, small, hardened] => Ok(Switches {
                writes_off: switch_on(writes),
                short_off: switch_on(short),
                writes_small_only: switch_on(small),
                resolve_hardened: switch_on(hardened),
            }),
            _ => Err(()),
        }
    }

    pub async fn set_switch(&self, name: &str, enabled: bool) -> Result<(), ()> {
        if !SWITCH_NAMES.contains(&name) {
            return Err(());
        }
        let mut connection = self.manager.clone();
        if enabled {
            let _: String = self
                .timed(
                    redis::cmd("SET")
                        .arg(self.switch_key(name))
                        .arg("1")
                        .query_async(&mut connection),
                )
                .await?;
        } else {
            let _: i64 = self
                .timed(
                    redis::cmd("DEL")
                        .arg(self.switch_key(name))
                        .query_async(&mut connection),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn block_bucket(&self, bucket: &str, ttl: Duration) -> Result<(), ()> {
        let mut connection = self.manager.clone();
        let _: String = self
            .timed(
                redis::cmd("SET")
                    .arg(self.block_key(bucket))
                    .arg("1")
                    .arg("EX")
                    .arg(ttl.as_secs().max(1))
                    .query_async(&mut connection),
            )
            .await?;
        Ok(())
    }

    pub async fn unblock_bucket(&self, bucket: &str) -> Result<bool, ()> {
        let mut connection = self.manager.clone();
        let removed: i64 = self
            .timed(
                redis::cmd("DEL")
                    .arg(self.block_key(bucket))
                    .query_async(&mut connection),
            )
            .await?;
        Ok(removed == 1)
    }

    pub async fn blocked_bucket_count(&self) -> Result<u64, ()> {
        Ok(self
            .scan_keys(&format!("{}block:*", self.prefix))
            .await?
            .len() as u64)
    }

    pub async fn meter_bytes(&self) -> Result<u64, ()> {
        let mut connection = self.manager.clone();
        let value: Option<String> = self
            .timed(
                redis::cmd("GET")
                    .arg(self.meter_key())
                    .query_async(&mut connection),
            )
            .await?;
        match value {
            None => Ok(0),
            Some(raw) => raw.parse::<i64>().map(|v| v.max(0) as u64).map_err(|_| ()),
        }
    }
    /// Recomputes the storage meter from actual notes. A revision compare-set
    /// prevents the scan from overwriting concurrent commit/delete/reveal
    /// mutations. Expiry-only races can make a snapshot conservatively high,
    /// never low, and the next cadence corrects them.
    pub async fn resync_meter(&self) -> Result<MeterSnapshot, ()> {
        for _ in 0..4 {
            let mut connection = self.manager.clone();
            let revision: Option<u64> = self
                .timed(
                    redis::cmd("GET")
                        .arg(self.meter_revision_key())
                        .query_async(&mut connection),
                )
                .await?;
            let revision = revision.unwrap_or(0);
            let keys = self.scan_keys(&format!("{}note:*", self.prefix)).await?;
            let mut total_bytes: u64 = 0;
            let mut notes: u64 = 0;
            for key in keys {
                let length: i64 = self
                    .timed(
                        redis::cmd("HSTRLEN")
                            .arg(&key)
                            .arg("envelope")
                            .query_async(&mut connection),
                    )
                    .await?;
                if length > 0 {
                    total_bytes = total_bytes
                        .checked_add((length as u64).saturating_mul(3) / 4)
                        .ok_or(())?;
                    notes += 1;
                }
            }
            let applied: i64 = self
                .timed(
                    redis::Script::new(RESYNC_METER_SCRIPT)
                        .key(self.meter_key())
                        .key(self.meter_revision_key())
                        .arg(revision)
                        .arg(total_bytes)
                        .invoke_async(&mut connection),
                )
                .await?;
            if applied == 1 {
                return Ok(MeterSnapshot {
                    bytes: total_bytes,
                    notes,
                });
            }
        }
        Err(())
    }

    /// Operator revocation: deletes a note (and its short code) without a
    /// delete token. Returns true when the note existed.
    pub async fn revoke(&self, id: &str) -> Result<bool, ()> {
        let mut connection = self.manager.clone();
        let removed: i64 = self
            .timed(
                redis::Script::new(REVOKE_SCRIPT)
                    .key(self.note_key(id))
                    .key(self.meter_key())
                    .key(self.meter_revision_key())
                    .arg(&self.prefix)
                    .invoke_async(&mut connection),
            )
            .await?;
        Ok(removed == 1)
    }

    /// Reads recent per-minute operation counters (newest first), for
    /// `nyanbin-admin stats`. Counter values only; no client identifiers.
    pub async fn recent_counters(
        &self,
        operation: &str,
        minutes: u64,
    ) -> Result<Vec<(u64, u64)>, ()> {
        let mut connection = self.manager.clone();
        let now: (u64, u64) = self
            .timed(redis::cmd("TIME").query_async(&mut connection))
            .await?;
        let current_minute = now.0 / 60;
        let mut output = Vec::with_capacity(minutes as usize);
        for offset in 0..minutes {
            let minute = current_minute.saturating_sub(offset);
            let key = format!("{}ctr:{operation}:{minute}", self.prefix);
            let value: Option<u64> = self
                .timed(redis::cmd("GET").arg(&key).query_async(&mut connection))
                .await?;
            output.push((minute, value.unwrap_or(0)));
        }
        Ok(output)
    }

    /// Top rate-limited pseudonymous buckets for the current and previous
    /// hour: the operator-facing view for `nyanbin-admin block`.
    pub async fn top_rejected_buckets(
        &self,
        operation: &str,
        limit: usize,
    ) -> Result<Vec<(String, u64)>, ()> {
        let mut connection = self.manager.clone();
        let now: (u64, u64) = self
            .timed(redis::cmd("TIME").query_async(&mut connection))
            .await?;
        let hour = now.0 / 3600;
        let mut merged: Vec<(String, u64)> = Vec::new();
        for h in [hour, hour.saturating_sub(1)] {
            let key = format!("{}rej:{operation}:{h}", self.prefix);
            let entries: Vec<(String, u64)> = self
                .timed(
                    redis::cmd("ZREVRANGE")
                        .arg(&key)
                        .arg(0)
                        .arg(limit.saturating_sub(1))
                        .arg("WITHSCORES")
                        .query_async(&mut connection),
                )
                .await?;
            for (bucket, count) in entries {
                match merged.iter_mut().find(|(b, _)| *b == bucket) {
                    Some((_, total)) => *total += count,
                    None => merged.push((bucket, count)),
                }
            }
        }
        merged.sort_by(|a, b| b.1.cmp(&a.1));
        merged.truncate(limit);
        Ok(merged)
    }

    async fn scan_keys(&self, pattern: &str) -> Result<Vec<String>, ()> {
        let mut connection = self.manager.clone();
        let mut cursor: u64 = 0;
        let mut output = Vec::new();
        loop {
            let (next, keys): (u64, Vec<String>) = self
                .timed(
                    redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(pattern)
                        .arg("COUNT")
                        .arg(200)
                        .query_async(&mut connection),
                )
                .await?;
            output.extend(keys);
            cursor = next;
            if cursor == 0 {
                return Ok(output);
            }
        }
    }
}

fn switch_on(value: &Option<String>) -> bool {
    value.as_deref() == Some("1")
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
    fn reveal_delete_and_revoke_scripts_decrement_the_meter() {
        // Decoded size of unpadded base64: floor(len * 3 / 4).
        assert!(REVEAL_SCRIPT.contains("math.floor(string.len(v[2]) * 3 / 4)"));
        assert!(DELETE_SCRIPT.contains("math.floor(string.len(envelope) * 3 / 4)"));
        assert!(REVOKE_SCRIPT.contains("math.floor(string.len(v[1]) * 3 / 4)"));
    }
    #[test]
    fn commit_script_orders_small_only_pressure_quota_write() {
        let small_only = COMMIT_SCRIPT.find("KEYS[4]").unwrap();
        let pressure = COMMIT_SCRIPT.find("tonumber(ARGV[9])").unwrap();
        let quota = COMMIT_SCRIPT.find("'bucket'").unwrap();
        let write = COMMIT_SCRIPT.find("redis.call('HSET', KEYS[2]").unwrap();
        assert!(small_only < pressure && pressure < quota && quota < write);
    }
    #[test]
    fn commit_script_meters_decoded_bytes_after_write() {
        let write = COMMIT_SCRIPT.find("redis.call('HSET', KEYS[2]").unwrap();
        let meter = COMMIT_SCRIPT.find("redis.call('INCRBY', KEYS[3]").unwrap();
        assert!(write < meter);
    }
    #[test]
    fn short_create_script_gates_on_password_protection() {
        let gate = SHORT_CREATE_SCRIPT
            .find("if v[3] ~= '1' then return {'unprotected'} end")
            .unwrap();
        let set = SHORT_CREATE_SCRIPT
            .find("redis.call('SET', KEYS[2]")
            .unwrap();
        assert!(gate < set);
    }
    #[test]
    fn short_resolve_script_self_heals_dead_notes() {
        assert!(SHORT_RESOLVE_SCRIPT.contains("redis.call('DEL', KEYS[1])"));
        assert!(SHORT_RESOLVE_SCRIPT.contains("return {'ok', id}"));
    }
    #[test]
    fn short_resolve_tripwire_arms_on_miss_surge_only() {
        assert!(SHORT_RESOLVE_SCRIPT.contains("misses >= 30 and misses > 4 * hits"));
        assert!(SHORT_RESOLVE_SCRIPT.contains("'EX', 600"));
        // Counters carry only the minute stamp, never an identifier.
        assert!(SHORT_RESOLVE_SCRIPT.contains("bump('short_resolve_miss')"));
        assert!(SHORT_RESOLVE_SCRIPT.contains("ctr:short_resolve_hit"));
        assert!(SHORT_RESOLVE_SCRIPT.contains(".. minute"));
    }
    #[test]
    fn rate_limit_script_returns_ttl_and_supports_switches() {
        assert!(RATE_LIMIT_SCRIPT.contains("return {0, ttl}"));
        assert!(RATE_LIMIT_SCRIPT.contains("return {1, 0}"));
        assert!(RATE_LIMIT_SCRIPT.contains("return {-2, 0}"));
        // Blocked buckets are refused before any counter increments.
        let block = RATE_LIMIT_SCRIPT.find("KEYS[3]").unwrap();
        let incr = RATE_LIMIT_SCRIPT
            .find("redis.call('INCR', KEYS[1])")
            .unwrap();
        assert!(block < incr);
    }
    #[test]
    fn switch_values_require_exact_flag() {
        assert!(switch_on(&Some("1".into())));
        assert!(!switch_on(&Some("0".into())));
        assert!(!switch_on(&Some("yes".into())));
        assert!(!switch_on(&None));
    }
    #[test]
    fn switch_names_match_documented_surface() {
        assert!(SWITCH_NAMES.contains(&"writes_off"));
        assert!(SWITCH_NAMES.contains(&"short_off"));
        assert!(SWITCH_NAMES.contains(&"writes_small_only"));
        assert!(SWITCH_NAMES.contains(&"resolve_hardened"));
    }
}
