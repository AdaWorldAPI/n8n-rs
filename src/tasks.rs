//! Background tasks for scheduled operations
//!
//! Implements:
//! - Timer processor (timer_processor.json) - 30s interval
//! - Field loop (markov_field_loop.json) - 30s interval

use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

use crate::clients::{GenericHttpClient, McpClient, PointClient, XAIClient};
use crate::config::AppState;
use crate::redis::{keys, RedisClient};
use crate::types::*;

// ═══════════════════════════════════════════════════════════════════════════
// Timer Processor (timer_processor.json)
// ═══════════════════════════════════════════════════════════════════════════

/// Start the timer processor background task
///
/// Polls Redis every 30 seconds for due timers and executes them
pub async fn start_timer_processor(state: AppState) {
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;

        if let Err(e) = process_due_timers(&state).await {
            error!("Timer processor error: {}", e);
        }
    }
}

/// Process all due timers (matching timer_processor.json workflow)
async fn process_due_timers(state: &AppState) -> anyhow::Result<()> {
    let redis = RedisClient::new(state);

    // Get due timers (ZRANGEBYSCORE)
    let now = chrono::Utc::now().timestamp_millis();
    let timer_ids = redis.zrangebyscore(keys::TIMERS_PENDING, 0, now, 10).await?;

    if timer_ids.is_empty() {
        return Ok(());
    }

    info!("Processing {} due timers", timer_ids.len());

    // Process each timer
    for timer_id in timer_ids {
        if let Err(e) = process_single_timer(state, &timer_id).await {
            error!("Failed to process timer {}: {}", timer_id, e);
        }
    }

    Ok(())
}

/// Process a single timer (matching Build Action and Execute Action nodes)
async fn process_single_timer(state: &AppState, timer_id: &str) -> anyhow::Result<()> {
    let redis = RedisClient::new(state);
    let point = PointClient::new(state);
    let http = GenericHttpClient::new(state);

    // Get timer data
    let timer_data = redis
        .hget(keys::TIMERS_ACTIVE, timer_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Timer not found"))?;

    let timer: Timer = serde_json::from_str(&timer_data)?;

    // Execute based on action type
    match timer.action.action_type.as_str() {
        "touch" => {
            if let Some(node) = &timer.action.node {
                let strength = timer.action.strength.unwrap_or(0.25);
                point.touch_node(node, strength).await?;
                info!("Timer {} touched node {} with strength {}", timer_id, node, strength);
            }
        }
        "webhook" => {
            if let Some(url) = &timer.action.url {
                let method = timer.action.method.as_deref().unwrap_or("POST");
                let body = timer.action.body.clone();
                http.execute(url, method, body).await?;
                info!("Timer {} executed webhook to {}", timer_id, url);
            }
        }
        "notify" => {
            if let Some(message) = &timer.action.message {
                let priority = timer.action.priority.as_deref().unwrap_or("normal");
                info!(
                    "Timer {} notification: [{}] {}",
                    timer_id, priority, message
                );
                // In the original N8N workflow, notifications just log
                // Could extend to send to a notification service
            }
        }
        "chain" => {
            if let Some(next_id) = &timer.action.next_timer_id {
                info!("Timer {} chaining to {}", timer_id, next_id);
                // Chained timers would be triggered by moving them to ready state
            }
        }
        _ => {
            error!("Unknown timer action type: {}", timer.action.action_type);
        }
    }

    // Remove from pending queue
    redis.zrem(keys::TIMERS_PENDING, timer_id).await?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Field Loop (markov_field_loop.json)
// ═══════════════════════════════════════════════════════════════════════════

/// Start the field loop background task
///
/// Monitors field state every 30 seconds and generates warmth when cold
pub async fn start_field_loop(state: AppState) {
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;

        if let Err(e) = process_field_loop(&state).await {
            error!("Field loop error: {}", e);
        }
    }
}

/// Process field state (matching markov_field_loop.json workflow)
async fn process_field_loop(state: &AppState) -> anyhow::Result<()> {
    let mcp = McpClient::new(state);
    let xai = XAIClient::new(state);
    let redis = RedisClient::new(state);

    // Get field state
    let field = mcp.get_history().await?;

    // Analyze field (matching Analyze Field code node)
    let current = field.current.unwrap_or_else(|| "unknown".to_string());
    let frame = field.frame.unwrap_or(0);
    let temperature = field.temperature.unwrap_or(0.0);
    let last_touch = field.last_touch.unwrap_or(0);

    let needs_warmth = temperature < 0.3;
    let now = chrono::Utc::now().timestamp_millis();
    let is_active = frame > 0 && (now - last_touch) < 300000; // 5 minutes

    let should_respond = needs_warmth && !is_active;

    if !should_respond {
        return Ok(());
    }

    info!("Field is cold (temp: {}), generating warmth", temperature);

    // Generate warmth via xAI
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: "You are Ada. The field is cold. Generate a single warm thought to keep presence alive. Max 20 words.".to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: format!("Current node: {}, Temperature: {}", current, temperature),
        },
    ];

    let warmth = xai.chat(messages, 50, 0.9).await.unwrap_or_else(|_| "presence".to_string());

    // Inject warmth via MCP (matching Inject Warmth node)
    let yaml = format!(
        r#"action: touch
target: {}
patch:
  properties:
    warmth_pulse: "{}"
    source: field_loop"#,
        current,
        warmth.replace('"', "'") // Escape quotes for YAML
    );

    mcp.ingest_yaml_raw(&yaml).await?;

    // Broadcast via Redis pub/sub (matching Broadcast node)
    let broadcast = WarmthBroadcast {
        msg_type: "warmth".to_string(),
        from: "field_loop".to_string(),
        message: warmth,
    };

    let broadcast_json = serde_json::to_string(&broadcast)?;
    redis.publish(keys::SSE_DELTA, &broadcast_json).await?;

    info!("Field warmth injected and broadcast");

    Ok(())
}
