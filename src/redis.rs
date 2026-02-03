//! Redis client for Upstash REST API
//!
//! Implements the Redis operations used in N8N workflows:
//! - HSET/HGET/HDEL for timer storage
//! - ZADD/ZRANGEBYSCORE/ZREM for timer queues
//! - LRANGE/RPUSH/LTRIM for chat history
//! - PUBLISH for event broadcasting

use crate::config::AppState;
use crate::types::RedisResponse;
use anyhow::{Context, Result};
use serde_json::Value;

/// Redis client wrapper for Upstash REST API
pub struct RedisClient<'a> {
    state: &'a AppState,
}

impl<'a> RedisClient<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    /// Execute a Redis command via Upstash REST API
    async fn execute(&self, command: Vec<Value>) -> Result<Value> {
        let response = self
            .state
            .http_client
            .post(&self.state.config.redis_url)
            .header(
                "Authorization",
                format!("Bearer {}", self.state.config.redis_token),
            )
            .json(&command)
            .send()
            .await
            .context("Failed to send Redis request")?;

        let redis_response: RedisResponse = response
            .json()
            .await
            .context("Failed to parse Redis response")?;

        Ok(redis_response.result)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Hash operations (timer storage)
    // ═══════════════════════════════════════════════════════════════════════

    /// HSET - Store timer in hash
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<Value> {
        self.execute(vec![
            Value::String("HSET".to_string()),
            Value::String(key.to_string()),
            Value::String(field.to_string()),
            Value::String(value.to_string()),
        ])
        .await
    }

    /// HGET - Get timer from hash
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let result = self
            .execute(vec![
                Value::String("HGET".to_string()),
                Value::String(key.to_string()),
                Value::String(field.to_string()),
            ])
            .await?;

        match result {
            Value::String(s) => Ok(Some(s)),
            Value::Null => Ok(None),
            _ => Ok(None),
        }
    }

    /// HDEL - Delete timer from hash
    pub async fn hdel(&self, key: &str, field: &str) -> Result<Value> {
        self.execute(vec![
            Value::String("HDEL".to_string()),
            Value::String(key.to_string()),
            Value::String(field.to_string()),
        ])
        .await
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Sorted set operations (timer queue)
    // ═══════════════════════════════════════════════════════════════════════

    /// ZADD - Add timer to pending queue
    pub async fn zadd(&self, key: &str, score: i64, member: &str) -> Result<Value> {
        self.execute(vec![
            Value::String("ZADD".to_string()),
            Value::String(key.to_string()),
            Value::String(score.to_string()),
            Value::String(member.to_string()),
        ])
        .await
    }

    /// ZRANGEBYSCORE - Get due timers
    pub async fn zrangebyscore(
        &self,
        key: &str,
        min: i64,
        max: i64,
        limit: usize,
    ) -> Result<Vec<String>> {
        let result = self
            .execute(vec![
                Value::String("ZRANGEBYSCORE".to_string()),
                Value::String(key.to_string()),
                Value::String(min.to_string()),
                Value::String(max.to_string()),
                Value::String("LIMIT".to_string()),
                Value::String("0".to_string()),
                Value::String(limit.to_string()),
            ])
            .await?;

        match result {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()),
            _ => Ok(vec![]),
        }
    }

    /// ZREM - Remove timer from pending queue
    pub async fn zrem(&self, key: &str, member: &str) -> Result<Value> {
        self.execute(vec![
            Value::String("ZREM".to_string()),
            Value::String(key.to_string()),
            Value::String(member.to_string()),
        ])
        .await
    }

    // ═══════════════════════════════════════════════════════════════════════
    // List operations (chat history)
    // ═══════════════════════════════════════════════════════════════════════

    /// LRANGE - Get chat history
    pub async fn lrange(&self, key: &str, start: i64, stop: i64) -> Result<Vec<String>> {
        let result = self
            .execute(vec![
                Value::String("LRANGE".to_string()),
                Value::String(key.to_string()),
                Value::String(start.to_string()),
                Value::String(stop.to_string()),
            ])
            .await?;

        match result {
            Value::Array(arr) => Ok(arr
                .into_iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()),
            _ => Ok(vec![]),
        }
    }

    /// RPUSH - Append to chat history
    pub async fn rpush(&self, key: &str, values: Vec<String>) -> Result<Value> {
        let mut command = vec![
            Value::String("RPUSH".to_string()),
            Value::String(key.to_string()),
        ];
        for v in values {
            command.push(Value::String(v));
        }
        self.execute(command).await
    }

    /// LTRIM - Trim chat history
    pub async fn ltrim(&self, key: &str, start: i64, stop: i64) -> Result<Value> {
        self.execute(vec![
            Value::String("LTRIM".to_string()),
            Value::String(key.to_string()),
            Value::String(start.to_string()),
            Value::String(stop.to_string()),
        ])
        .await
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Pub/Sub operations (broadcasting)
    // ═══════════════════════════════════════════════════════════════════════

    /// PUBLISH - Broadcast message
    pub async fn publish(&self, channel: &str, message: &str) -> Result<Value> {
        self.execute(vec![
            Value::String("PUBLISH".to_string()),
            Value::String(channel.to_string()),
            Value::String(message.to_string()),
        ])
        .await
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Redis key constants (from timer_dto.yaml)
// ═══════════════════════════════════════════════════════════════════════════

pub mod keys {
    pub const TIMERS_PENDING: &str = "ada:timers:pending";
    pub const TIMERS_ACTIVE: &str = "ada:timers:active";
    pub const SSE_DELTA: &str = "sse:delta";

    pub fn chat_history(session: &str) -> String {
        format!("chat:markov:{}", session)
    }

    #[allow(dead_code)]
    pub fn timer_history(id: &str) -> String {
        format!("ada:timers:history:{}", id)
    }
}
