use crate::db::Database;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// 用于在服务器和客户端之间同步的结构
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncPackage {
    pub agent_id: String,
    pub connections: Vec<ConnectionRecord>,
    pub timestamp: DateTime<Utc>,
}

// 数据库中存储的连接记录
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionRecord {
    pub id: String,
    pub download: i64,
    pub upload: i64,
    pub last_updated: String,
    pub start: String,
    pub network: String,
    pub source_ip: String,
    pub destination_ip: String,
    pub source_port: String,
    pub destination_port: String,
    pub host: String,
    pub process: String,
    pub process_path: String,
    pub special_rules: String,
    pub chains: String,
    pub rule: String,
    pub rule_payload: String,
    pub agent_id: Option<String>,
}

impl From<&ConnectionRecord> for ConnectionLog {
    fn from(record: &ConnectionRecord) -> Self {
        ConnectionLog {
            id: record.id.clone(),
            agent_id: record.agent_id.clone().unwrap_or_default(),
            source_ip: record.source_ip.clone(),
            destination_ip: record.destination_ip.clone(),
            source_port: record.source_port.clone(),
            destination_port: record.destination_port.clone(),
            host: record.host.clone(),
            rule: record.rule.clone(),
            rule_payload: record.rule_payload.clone(),
            chains: record.chains.clone(),
            network: record.network.clone(),
            process: record.process.clone(),
            process_path: record.process_path.clone(),
            download: record.download,
            upload: record.upload,
            start: record.start.clone(),
            // 注意：ConnectionRecord 不记录连接关闭时间，此处使用当前时间戳作为近似值
            // 真实关闭时间应由 Mihomo API 的断连事件提供，但当前数据模型未捕获
            end: Utc::now().to_rfc3339(),
            special_rules: record.special_rules.clone(),
            synced: None,
        }
    }
}

// 已关闭连接的流量审计日志
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionLog {
    pub id: String,
    pub agent_id: String,
    pub source_ip: String,
    pub destination_ip: String,
    pub source_port: String,
    pub destination_port: String,
    pub host: String,
    pub rule: String,
    pub rule_payload: String,
    pub chains: String,
    pub network: String,
    pub process: String,
    pub process_path: String,
    pub download: i64,
    pub upload: i64,
    pub start: String,
    pub end: String,
    pub special_rules: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced: Option<i32>,
}

// 从 Mihomo API 接收的数据结构
#[derive(Deserialize, Debug, Clone)]
pub struct GlobalData {
    #[serde(deserialize_with = "deserialize_connections")]
    pub connections: Vec<Connection>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Connection {
    pub id: String,
    pub download: i64,
    pub upload: i64,
    pub start: String, // ISO8601 string
    pub metadata: ConnectionMetadata,
    #[serde(deserialize_with = "deserialize_chains")]
    pub chains: Vec<String>,
    pub rule: String,
    #[serde(rename = "rulePayload")]
    pub rule_payload: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConnectionMetadata {
    pub network: String,
    #[serde(rename = "sourceIP")]
    pub source_ip: String,
    #[serde(rename = "destinationIP")]
    pub destination_ip: String,
    #[serde(rename = "sourcePort")]
    pub source_port: String,
    #[serde(rename = "destinationPort")]
    pub destination_port: String,
    pub host: String,
    pub process: String,
    #[serde(rename = "processPath")]
    pub process_path: String,
    #[serde(rename = "specialRules")]
    pub special_rules: String,
}

// 连接跟踪器的状态
#[derive(Debug)]
pub struct ConnectionState {
    pub flow_cache: HashMap<String, (i64, i64)>, // id -> (download, upload)
    pub record_cache: HashMap<String, ConnectionRecord>, // id -> latest snapshot
    pub active_connections: HashSet<String>,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self {
            flow_cache: HashMap::new(),
            record_cache: HashMap::new(),
            active_connections: HashSet::new(),
        }
    }
}

// 从 Connection 转换为 ConnectionRecord
pub fn connection_to_record(conn: &Connection, agent_id: Option<String>) -> ConnectionRecord {
    let last_updated = Utc::now().to_rfc3339();
    let chains = serde_json::to_string(&conn.chains).unwrap_or_else(|e| {
        tracing::warn!("序列化 chains 失败: {:?}, 使用默认值", e);
        "[]".to_string()
    });

    let start = if conn.start.ends_with('Z')
        || conn.start.contains('+')
        || (conn.start.matches('-').count() > 1 && conn.start.contains('T'))
    {
        conn.start.clone()
    } else {
        match chrono::DateTime::parse_from_str(&conn.start, "%Y-%m-%dT%H:%M:%S%.f") {
            Ok(dt) => dt.with_timezone(&Utc).to_rfc3339(),
            Err(_) => conn.start.clone(),
        }
    };

    ConnectionRecord {
        id: conn.id.clone(),
        download: conn.download,
        upload: conn.upload,
        last_updated,
        start,
        network: conn.metadata.network.clone(),
        source_ip: conn.metadata.source_ip.clone(),
        destination_ip: conn.metadata.destination_ip.clone(),
        source_port: conn.metadata.source_port.clone(),
        destination_port: conn.metadata.destination_port.clone(),
        host: conn.metadata.host.clone(),
        process: conn.metadata.process.clone(),
        process_path: conn.metadata.process_path.clone(),
        special_rules: conn.metadata.special_rules.clone(),
        chains,
        // rule 字段：Mihomo 中 chains[0] 是出口节点（最后执行的规则），优先取 chains[0]，
        // 若 chains 为空则回退到 conn.rule（全局规则）
        rule: conn
            .chains
            .first()
            .cloned()
            .unwrap_or_else(|| conn.rule.clone()),
        rule_payload: conn.rule_payload.clone(),
        agent_id,
    }
}

pub fn deserialize_connections<'de, D>(deserializer: D) -> Result<Vec<Connection>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<Vec<Connection>>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

pub fn deserialize_chains<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut chains: Vec<String> = Vec::deserialize(deserializer)?;
    chains.reverse();
    Ok(chains)
}

// 实时日志流事件
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogStreamEvent {
    System {
        timestamp: String,
        level: String,
        target: String,
        message: String,
    },
    ConnectionClosed {
        timestamp: String,
        connection: Box<ConnectionLog>,
    },
}

// 格式化流量大小为人类可读格式
pub fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// 处理连接更新的公共函数 - 主从节点都可以调用
pub fn process_connections(
    data: &GlobalData,
    state_lock: &mut ConnectionState,
    db: Arc<Database>,
    agent_id: Option<String>,
) -> Result<(), String> {
    let current_conn_ids: std::collections::HashSet<_> =
        data.connections.iter().map(|c| c.id.clone()).collect();

    let new_conns: Vec<_> = current_conn_ids
        .difference(&state_lock.active_connections)
        .cloned()
        .collect();

    if !new_conns.is_empty() {
        tracing::debug!(target: "backend::connections", "新连接 ({})", new_conns.len());
        for conn_id in &new_conns {
            if let Some(conn) = data.connections.iter().find(|c| &c.id == conn_id) {
                tracing::info!(
                    target: "backend::connections",
                    "  - {} {}:{} -> {}:{} (链路: {})",
                    conn.metadata.network,
                    conn.metadata.source_ip,
                    conn.metadata.source_port,
                    conn.metadata.destination_ip,
                    conn.metadata.destination_port,
                    conn.chains.join("->")
                );
            }
        }
    }

    let closed_conns: Vec<_> = state_lock
        .active_connections
        .difference(&current_conn_ids)
        .cloned()
        .collect();

    if !closed_conns.is_empty() {
        tracing::debug!(target: "backend::connections", "关闭连接 ({})", closed_conns.len());
        for conn_id in &closed_conns {
            if let Some(conn_info) = state_lock.flow_cache.get(conn_id) {
                let download = conn_info.0;
                let upload = conn_info.1;
                tracing::debug!(
                    target: "backend::connections",
                    "  - {} [↑: {}, ↓: {}, 总计: {}]",
                    conn_id,
                    format_bytes(upload),
                    format_bytes(download),
                    format_bytes(upload + download)
                );
            } else {
                tracing::debug!(target: "backend::connections", "  - {}", conn_id);
            }
            state_lock.flow_cache.remove(conn_id);
            state_lock.record_cache.remove(conn_id);
        }
    }

    let mut connections_to_update = Vec::new();
    let mut flow_changed_connections = Vec::new();

    for conn in &data.connections {
        let is_new_connection = new_conns.contains(&conn.id);

        let has_flow_change =
            if let Some((old_download, old_upload)) = state_lock.flow_cache.get(&conn.id) {
                *old_upload != conn.upload || *old_download != conn.download
            } else {
                true
            };

        if is_new_connection || has_flow_change {
            connections_to_update.push(conn);
        }

        if has_flow_change && !is_new_connection {
            flow_changed_connections.push((
                conn.metadata.network.clone(),
                format!("{}:{}", conn.metadata.source_ip, conn.metadata.source_port),
                format!(
                    "{}:{}",
                    conn.metadata.destination_ip, conn.metadata.destination_port
                ),
                conn.upload,
                conn.download,
            ));
        }

        state_lock
            .flow_cache
            .insert(conn.id.clone(), (conn.download, conn.upload));
        state_lock.record_cache.insert(
            conn.id.clone(),
            connection_to_record(conn, agent_id.clone()),
        );
    }

    if !flow_changed_connections.is_empty() {
        tracing::debug!(target: "backend::connections", "连接流量更新 ({})", flow_changed_connections.len());
        for (network, source, destination, upload, download) in &flow_changed_connections {
            let total = upload + download;
            tracing::info!(
                target: "backend::connections",
                "  - {} {} -> {} [↑: {}, ↓: {}, 总计: {}]",
                network,
                source,
                destination,
                format_bytes(*upload),
                format_bytes(*download),
                format_bytes(total)
            );
        }
    }

    state_lock.active_connections = current_conn_ids;

    if !connections_to_update.is_empty() || !closed_conns.is_empty() {
        let db_clone = db.clone();
        let agent_id_clone = agent_id.clone();

        let connections_to_update: Vec<Connection> = connections_to_update
            .iter()
            .map(|&conn| conn.clone())
            .collect();

        let ids_to_delete: Vec<(String, String)> = closed_conns
            .iter()
            .map(|id| (id.clone(), agent_id_clone.clone().unwrap_or_default()))
            .collect();

        let connections_to_update = connections_to_update.clone();
        let ids_to_delete = ids_to_delete.clone();
        let agent_id_for_update = agent_id_clone.unwrap_or_default();

        tokio::spawn(async move {
            if !connections_to_update.is_empty() {
                let records: Vec<_> = connections_to_update
                    .iter()
                    .map(|conn| connection_to_record(conn, Some(agent_id_for_update.clone())))
                    .collect();

                if let Err(e) = db_clone.batch_upsert_records(&records).await {
                    tracing::error!(target: "backend::connections", "批量更新连接数据错误: {}", e);
                } else {
                    tracing::debug!(target: "backend::connections", "更新了 {} 个连接记录", records.len());
                }
            }

            if !ids_to_delete.is_empty() {
                if let Err(e) = db_clone.batch_delete_records(&ids_to_delete).await {
                    tracing::error!(target: "backend::connections", "批量删除关闭连接错误: {}", e);
                } else {
                    tracing::debug!(target: "backend::connections", "删除了 {} 个关闭连接记录", ids_to_delete.len());
                }
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ConnectionLog, ConnectionRecord, ConnectionState};

    fn sample_record(agent_id: Option<String>) -> ConnectionRecord {
        ConnectionRecord {
            id: "conn-1".to_string(),
            download: 10,
            upload: 20,
            last_updated: "2026-01-01T00:00:00Z".to_string(),
            start: "2026-01-01T00:00:00Z".to_string(),
            network: "tcp".to_string(),
            source_ip: "127.0.0.1".to_string(),
            destination_ip: "1.1.1.1".to_string(),
            source_port: "12345".to_string(),
            destination_port: "443".to_string(),
            host: "example.com".to_string(),
            process: "test".to_string(),
            process_path: "".to_string(),
            special_rules: "".to_string(),
            chains: "[]".to_string(),
            rule: "DIRECT".to_string(),
            rule_payload: "".to_string(),
            agent_id,
        }
    }

    #[test]
    fn connection_state_starts_with_empty_caches() {
        let state = ConnectionState::new();
        assert!(state.active_connections.is_empty());
        assert!(state.flow_cache.is_empty());
        assert!(state.record_cache.is_empty());
    }

    #[test]
    fn connection_log_from_record_keeps_key_fields() {
        let record = sample_record(Some("agent-1".to_string()));
        let log = ConnectionLog::from(&record);

        assert_eq!(log.id, "conn-1");
        assert_eq!(log.agent_id, "agent-1");
        assert_eq!(log.download, 10);
        assert_eq!(log.upload, 20);
        assert_eq!(log.start, "2026-01-01T00:00:00Z");
        assert_eq!(log.synced, None);
        assert!(!log.end.is_empty());
    }
}
