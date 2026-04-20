use crate::api::{MasterClient, MihomoClient};
use crate::common::{process_connections, ConnectionRecord, ConnectionState, SyncPackage};
use crate::config::AgentConfig;
use crate::db::Database;
use chrono::Utc;
use std::error::Error;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::signal;
use tokio::time::interval;
use uuid::Uuid;

// 从节点状态
struct AgentState {
    db: Arc<Database>,
    conn_state: Arc<StdMutex<ConnectionState>>,
    master_client: Option<Arc<MasterClient>>,
    agent_id: String,
}

// 运行从节点客户端
pub async fn run(config: AgentConfig) -> Result<(), Box<dyn Error>> {
    tracing::info!(target: "backend::agent", "初始化从节点数据库...");
    let database = Arc::new(Database::new(&config.local_database).await?);

    // 生成或使用提供的节点ID
    let agent_id = config
        .agent_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let mode_info = if let Some(master_url) = &config.master_url {
        format!(
            "  - 主节点URL: {}\n  - 主节点认证: {}",
            master_url,
            config.master_token.as_ref().map_or("未启用", |_| "已启用")
        )
    } else {
        "  - 离线模式: 数据仅存储在本地".to_string()
    };
    tracing::info!(
        target: "backend::agent",
        "配置信息:\n  - 节点ID: {}\n  - Mihomo API: {}:{}\n  - 本地数据库: {}\n{}\n  - 同步间隔: {}秒\n  - 数据保留天数: {}天",
        agent_id,
        config.mihomo_host,
        config.mihomo_port,
        config.local_database,
        mode_info,
        config.sync_interval,
        config.data_retention_days
    );

    // 创建MihomoAPI客户端
    let mihomo_client = MihomoClient::new(
        config.mihomo_host.clone(),
        config.mihomo_port,
        config.mihomo_token.clone(),
    );

    // 创建从节点状态
    let conn_state = Arc::new(StdMutex::new(ConnectionState::new()));

    // 创建主节点API客户端（如果配置了主节点）
    let master_client: Option<Arc<MasterClient>> = config
        .master_url
        .as_ref()
        .map(|url| MasterClient::new(url.clone(), config.master_token.clone()).map(Arc::new))
        .transpose()?;

    // 创建从节点状态
    let state = Arc::new(AgentState {
        db: database.clone(),
        conn_state: conn_state.clone(),
        master_client: master_client.clone(),
        agent_id: agent_id.clone(),
    });

    // 启动同步任务（如果配置了主节点）
    let sync_handle = if config.master_url.is_some() {
        let sync_state = state.clone();
        let sync_interval = config.sync_interval;
        Some(tokio::spawn(async move {
            if let Err(e) = sync_task(sync_state, sync_interval).await {
                tracing::error!(target: "backend::agent::sync", "同步任务错误: {}", e);
            }
        }))
    } else {
        None
    };

    // 启动清理任务（每小时执行一次）
    let cleanup_handle = {
        let cleanup_state = state.clone();
        let data_retention = config.data_retention_days;
        let log_retention = config.log_retention_days;
        Some(tokio::spawn(async move {
            if let Err(e) = cleanup_task(cleanup_state, data_retention, log_retention).await {
                tracing::error!(target: "backend::agent::cleanup", "清理任务错误: {}", e);
            }
        }))
    };

    tracing::info!(target: "backend::agent", "从节点客户端已启动，按Ctrl+C关闭");

    // 创建关闭连接审计日志 channel 和 worker，避免 fire-and-forget 导致错误丢失
    let (closed_tx, mut closed_rx) = tokio::sync::mpsc::channel::<Vec<ConnectionRecord>>(4096);
    let worker_db = database.clone();
    let worker_master_client = master_client.clone();
    let worker_has_master = config.master_url.is_some();
    let closed_worker_handle = tokio::spawn(async move {
        while let Some(records) = closed_rx.recv().await {
            if let Err(e) = persist_closed_connection_logs(
                worker_db.clone(),
                worker_master_client.clone(),
                worker_has_master,
                records,
            )
            .await
            {
                tracing::error!(target: "backend::agent::logs", "审计日志 worker 处理失败: {:?}", e);
            }
        }
        tracing::debug!(target: "backend::agent::logs", "审计日志 worker 已结束");
    });

    // 启动连接处理（带重试循环）
    let mihomo_client_for_connect = mihomo_client.clone();
    let state_for_connect = state.clone();
    let closed_tx_for_connect_loop = closed_tx.clone();
    let mut connect_handle = tokio::spawn(async move {
        const MAX_RETRIES: u32 = 10;
        let mut retry_count: u32 = 0;
        let mut backoff_secs: u64 = 5;

        loop {
            let tx = closed_tx_for_connect_loop.clone();
            let state_for_callback = state_for_connect.clone();
            let connect_result: Result<(), String> = mihomo_client_for_connect.connect(move |data| {
                // 同步处理连接数据
                let state_ref = &state_for_callback;
                let db = &state_ref.db;
                let agent_id = Some(state_ref.agent_id.clone());

                // 获取并锁定当前状态
                let mut state_lock = match state_ref.conn_state.lock() {
                    Ok(lock) => lock,
                    Err(e) => {
                        tracing::error!(target: "backend::agent::mihomo", "连接状态锁被污染，尝试恢复: {:?}", e);
                        e.into_inner()
                    }
                };

                // 在状态变更前计算关闭连接 ID（用于异步审计落库）
                let current_conn_ids: std::collections::HashSet<_> = data.connections
                    .iter()
                    .map(|conn| conn.id.clone())
                    .collect();
                let closed_conn_ids: Vec<String> = state_lock
                    .active_connections
                    .difference(&current_conn_ids)
                    .cloned()
                    .collect();
                let closed_records: Vec<ConnectionRecord> = closed_conn_ids
                    .iter()
                    .filter_map(|id| state_lock.record_cache.get(id).cloned())
                    .collect();
                let missing_closed = closed_conn_ids.len().saturating_sub(closed_records.len());
                if missing_closed > 0 {
                    tracing::warn!(
                        target: "backend::agent::logs",
                        "检测到 {} 条关闭连接缺少内存快照，可能影响审计日志完整性",
                        missing_closed
                    );
                }

                // 使用公共函数处理连接更新
                if let Err(e) = process_connections(&data, &mut state_lock, db.clone(), agent_id.clone()) {
                    tracing::error!(target: "backend::agent::mihomo", "处理连接数据失败: {}", e);
                    return Err(format!("处理连接数据失败: {}", e));
                }

                if !closed_records.is_empty() {
                    let tx_for_blocking = tx.clone();
                    match tx.try_send(closed_records) {
                        Ok(()) => {}
                        Err(tokio::sync::mpsc::error::TrySendError::Full(records)) => {
                            tracing::warn!(target: "backend::agent::logs", "审计日志队列已满，尝试阻塞发送...");
                            if let Err(e) = tx_for_blocking.blocking_send(records) {
                                tracing::error!(target: "backend::agent::logs", "阻塞发送也失败，丢弃 {} 条关闭连接记录: {:?}", e.0.len(), e);
                            }
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            tracing::error!(target: "backend::agent::logs", "审计日志 worker 已关闭");
                        }
                    }
                }

                Ok(())
            }).await.map_err(|e| e.to_string());

            match connect_result {
                Ok(()) => {
                    tracing::info!(target: "backend::agent", "Mihomo 连接正常断开");
                    break;
                }
                Err(msg) => {
                    retry_count += 1;
                    if retry_count > MAX_RETRIES {
                        tracing::error!(target: "backend::agent", "Mihomo 连接重试次数超过上限 ({}), 放弃重连: {}", MAX_RETRIES, msg);
                        break;
                    }
                    tracing::warn!(target: "backend::agent", "Mihomo 连接异常，{}秒后重试 (第{}/{}次): {}", backoff_secs, retry_count, MAX_RETRIES, msg);
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(300);
                }
            }
        }
    });

    // 启动WebSocket客户端并等待中断信号
    tokio::select! {
        result = &mut connect_handle => {
            if let Err(e) = result {
                tracing::error!(target: "backend::agent", "连接任务异常: {:?}", e);
            }
            Ok(())
        }
        // 监听中断信号
        _ = signal::ctrl_c() => {
            tracing::info!(target: "backend::agent", "收到关闭信号，正在关闭客户端...");
            connect_handle.abort();
            if let Some(h) = sync_handle { h.abort(); }
            if let Some(h) = cleanup_handle { h.abort(); }
            drop(closed_tx);
            match tokio::time::timeout(Duration::from_secs(5), closed_worker_handle).await {
                Ok(Ok(())) => tracing::debug!(target: "backend::agent::logs", "审计日志 worker 正常结束"),
                Ok(Err(e)) => tracing::error!(target: "backend::agent::logs", "审计日志 worker 异常: {:?}", e),
                Err(_) => {
                    tracing::warn!(target: "backend::agent::logs", "审计日志 worker 未能在 5 秒内完成，已离开后台自行结束");
                }
            }
            Ok(())
        }
    }
}

async fn persist_closed_connection_logs(
    db: Arc<Database>,
    master_client: Option<Arc<MasterClient>>,
    has_master: bool,
    closed_records: Vec<ConnectionRecord>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if closed_records.is_empty() {
        return Ok(());
    }

    let logs: Vec<crate::common::ConnectionLog> = closed_records
        .iter()
        .map(crate::common::ConnectionLog::from)
        .collect();

    db.batch_insert_connection_logs(&logs).await.map_err(|e| {
        tracing::error!(target: "backend::agent::logs", "批量写入 connection_log 失败: {:?}", e);
        Box::new(e) as Box<dyn Error + Send + Sync>
    })?;

    if has_master {
        if let Some(client) = master_client {
            for log in &logs {
                if let Err(e) = client.report_connection_closed(log).await {
                    tracing::debug!(target: "backend::agent::logs", "实时上报 connection_closed 失败 (id={}): {:?}", log.id, e);
                    // warn级别
                }
            }
        }
    }
    Ok(())
}

// 同步任务 - 定期将本地数据同步到主节点
async fn sync_task(
    state: Arc<AgentState>,
    interval_secs: u64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut interval = interval(Duration::from_secs(interval_secs));

    let client = match state.master_client.as_ref() {
        Some(c) => c,
        None => {
            tracing::debug!(target: "backend::agent::sync", "未配置主节点，跳过同步");
            return Ok(());
        }
    };

    loop {
        interval.tick().await;
        const BATCH_SIZE: i64 = 1000;

        // 检查主节点是否在线
        let master_online = match client.is_online().await {
            Ok(online) => online,
            Err(e) => {
                tracing::warn!(target: "backend::agent::sync", "主节点健康检查失败: {}", e);
                continue;
            }
        };
        if !master_online {
            tracing::debug!(target: "backend::agent::sync", "主节点离线，跳过同步...");
            continue;
        }

        // 同步活跃连接全量快照（connections 只保留当前活跃连接）
        match state.db.get_active_records(BATCH_SIZE).await {
            Ok(records) => {
                if records.is_empty() {
                    tracing::debug!(target: "backend::agent::sync", "没有活跃连接需要同步");
                } else {
                    tracing::debug!(target: "backend::agent::sync", "同步活跃连接全量快照，包含 {} 条记录", records.len());

                    let sync_package = SyncPackage {
                        agent_id: state.agent_id.clone(),
                        connections: records,
                        timestamp: Utc::now(),
                    };

                    if let Err(e) = client.sync_data(&sync_package).await {
                        tracing::error!(target: "backend::agent::sync", "同步活跃连接快照失败: {:?}", e);
                    } else {
                        tracing::debug!(target: "backend::agent::sync", "成功同步活跃连接快照");
                    }
                }
            }
            Err(e) => {
                tracing::error!(target: "backend::agent::sync", "获取活跃连接记录失败: {:?}", e);
            }
        }

        // 同步 connection_logs 审计日志
        loop {
            match state
                .db
                .get_pending_connection_logs(&state.agent_id, BATCH_SIZE)
                .await
            {
                Ok(logs) => {
                    if logs.is_empty() {
                        break;
                    }
                    tracing::debug!(target: "backend::agent::sync", "发现 {} 条待同步审计日志", logs.len());
                    match client.sync_connection_logs(&state.agent_id, &logs).await {
                        Ok(_) => {
                            let ids: Vec<(String, String)> =
                                logs.into_iter().map(|l| (l.id, l.agent_id)).collect();
                            if let Err(e) = state.db.mark_connection_logs_synced(&ids).await {
                                tracing::error!(target: "backend::agent::sync", "标记审计日志同步状态失败: {:?}", e);
                                break;
                            }
                            tracing::info!(target: "backend::agent::sync", "成功同步 {} 条审计日志", ids.len());
                        }
                        Err(e) => {
                            tracing::error!(target: "backend::agent::sync", "同步审计日志失败: {:?}", e);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(target: "backend::agent::sync", "获取待同步审计日志失败: {:?}", e);
                    break;
                }
            }
        }
    }
}

// 清理任务 - 每小时执行一次
async fn cleanup_task(
    state: Arc<AgentState>,
    data_retention_days: i64,
    log_retention_days: i64,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut ticker = interval(Duration::from_secs(60 * 60));

    loop {
        ticker.tick().await;

        // connections 兜底清理（防止删除失败残留）
        match state.db.cleanup_old_records(data_retention_days).await {
            Ok(deleted) => {
                if deleted > 0 {
                    tracing::debug!(target: "backend::agent::cleanup", "已清理 {} 条超过{}天的旧连接数据", deleted, data_retention_days);
                }
            }
            Err(e) => {
                tracing::error!(target: "backend::agent::cleanup", "清理旧连接数据失败: {:?}", e);
            }
        }

        // 立即清理已同步的 connection_logs
        match state.db.cleanup_synced_connection_logs().await {
            Ok(deleted) => {
                if deleted > 0 {
                    tracing::debug!(target: "backend::agent::cleanup", "已清理 {} 条已同步审计日志", deleted);
                }
            }
            Err(e) => {
                tracing::error!(target: "backend::agent::cleanup", "清理已同步审计日志失败: {:?}", e);
            }
        }

        // 强制清理未同步但超期的 connection_logs
        match state
            .db
            .cleanup_old_unsynced_connection_logs(log_retention_days)
            .await
        {
            Ok(deleted) => {
                if deleted > 0 {
                    tracing::warn!(target: "backend::agent::cleanup", "已清理 {} 条超过{}天未同步的旧审计日志", deleted, log_retention_days);
                }
            }
            Err(e) => {
                tracing::error!(target: "backend::agent::cleanup", "清理未同步审计日志失败: {:?}", e);
            }
        }

        // 清理后 VACUUM 回收磁盘空间
        if let Err(e) = state.db.vacuum_db().await {
            tracing::error!(target: "backend::agent::cleanup", "VACUUM 失败: {:?}", e);
        }
    }
}
