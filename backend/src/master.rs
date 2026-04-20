use crate::api::{publish_log_event, MasterServer, MihomoClient};
use crate::common::{
    process_connections, ConnectionLog, ConnectionRecord, ConnectionState, LogStreamEvent,
};
use crate::config::MasterConfig;
use crate::db::Database;
use chrono::Utc;
use std::error::Error;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::signal;
use tokio::sync::Mutex;

// 主节点状态
struct MasterState {
    db: Arc<Database>,
    conn_state: Arc<StdMutex<ConnectionState>>,
    closed_worker_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

// 运行主节点服务器
pub async fn run(config: MasterConfig) -> Result<(), Box<dyn Error>> {
    tracing::info!(target: "backend::master", "初始化主节点数据库...");
    let database = Arc::new(Database::new(&config.database).await?);

    tracing::info!(
        target: "backend::master",
        "配置信息:\n  - 数据库: {}\n  - 监听地址: {}:{}\n  - API认证: {}",
        config.database,
        config.listen_host,
        config.listen_port,
        config.api_token.as_ref().map_or("未启用", |_| "已启用")
    );

    // 如果配置了Mihomo连接信息，则显示
    if let Some(mihomo_host) = &config.mihomo_host {
        tracing::info!(
            target: "backend::master",
            "  - 本地Mihomo API: {}:{}\n  - Mihomo API认证: {}",
            mihomo_host,
            config.mihomo_port.unwrap_or(9090),
            config.mihomo_token.as_ref().map_or("未启用", |_| "已启用")
        );
    }

    // 创建主节点状态
    let conn_state = Arc::new(StdMutex::new(ConnectionState::new()));
    let state = Arc::new(MasterState {
        db: database.clone(),
        conn_state: conn_state.clone(),
        closed_worker_handle: Mutex::new(None),
    });

    // 如果配置了本地Mihomo API，则启动本地数据收集
    let mihomo_task = if let Some(mihomo_host) = &config.mihomo_host {
        let mihomo_port = config.mihomo_port.unwrap_or(9090);
        let mihomo_token = config.mihomo_token.clone().unwrap_or_default();
        let mihomo_client = MihomoClient::new(mihomo_host.clone(), mihomo_port, mihomo_token);

        // 克隆状态用于Mihomo API任务
        let mihomo_state = state.clone();

        tracing::info!(target: "backend::master", "启动本地Mihomo连接数据收集...");

        // 运行Mihomo数据收集任务（带重启循环和退避）
        Some(tokio::spawn(async move {
            loop {
                match run_mihomo_collection_loop(mihomo_client.clone(), mihomo_state.clone()).await
                {
                    Ok(()) => break,
                    Err(msg) => {
                        tracing::error!(target: "backend::master", "Mihomo数据收集错误，5秒后重试: {}", msg);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }))
    } else {
        None
    };

    // 启动 connections 兜底清理任务（每小时，保留最近 24 小时）
    let db_for_conn_cleanup = database.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60 * 60));
        ticker.tick().await;
        loop {
            match db_for_conn_cleanup.cleanup_stale_connections(1).await {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::debug!(target: "backend::master", "已清理 {} 条超过 1 天未更新的旧连接数据", deleted);
                    }
                }
                Err(e) => {
                    tracing::error!(target: "backend::master", "清理旧连接数据失败: {:?}", e);
                }
            }
            ticker.tick().await;
        }
    });

    // 启动数据保留清理任务
    let log_retention_days = config.log_retention_days;
    let db_for_cleanup = database.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        ticker.tick().await;
        loop {
            match db_for_cleanup
                .cleanup_old_connection_logs(log_retention_days)
                .await
            {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(target: "backend::master", "已清理 {} 条超过{}天的过期审计日志", deleted, log_retention_days);
                    }
                }
                Err(e) => {
                    tracing::error!(target: "backend::master", "清理过期审计日志失败: {:?}", e);
                }
            }
            ticker.tick().await;
        }
    });

    // 创建并启动API服务器
    let server = MasterServer::new(database, config.api_token);

    tracing::info!(target: "backend::master", "主节点服务器已启动，按Ctrl+C关闭");

    // 启动服务器并等待中断信号
    tokio::select! {
        result = server.start(&config.listen_host, config.listen_port) => {
            if let Err(e) = result {
                tracing::error!(target: "backend::master", "服务器错误: {:?}", e);
                return Err(e);
            }
            Ok(())
        }
        _ = signal::ctrl_c() => {
            // 如果存在Mihomo任务，强制中断它
            if let Some(mihomo_handle) = mihomo_task {
                mihomo_handle.abort();
            }
            // 等待审计日志 worker 完成剩余写入
            let worker = state.closed_worker_handle.lock().await.take();
            if let Some(worker) = worker {
                match tokio::time::timeout(Duration::from_secs(5), worker).await {
                    Ok(Ok(())) => tracing::debug!(target: "backend::master::logs", "审计日志 worker 正常结束"),
                    Ok(Err(e)) => tracing::error!(target: "backend::master::logs", "审计日志 worker 异常: {:?}", e),
                    Err(_) => tracing::warn!(target: "backend::master::logs", "审计日志 worker 等待超时"),
                }
            }
            tracing::info!(target: "backend::master", "收到关闭信号，正在关闭服务器...");
            Ok(())
        }
    }
}

// 运行本地Mihomo数据收集（返回 String 错误以便在 Send future 中使用）
async fn run_mihomo_collection_loop(
    mihomo_client: MihomoClient,
    state: Arc<MasterState>,
) -> Result<(), String> {
    run_mihomo_collection(mihomo_client, state)
        .await
        .map_err(|e| e.to_string())
}

// 运行本地Mihomo数据收集
async fn run_mihomo_collection(
    mihomo_client: MihomoClient,
    state: Arc<MasterState>,
) -> Result<(), Box<dyn Error>> {
    // 本地数据使用"local"作为agent_id
    let agent_id = Some("local".to_string());

    // 创建关闭连接审计日志 channel 和 worker，避免 fire-and-forget 导致错误丢失
    let (closed_tx, mut closed_rx) = tokio::sync::mpsc::channel::<Vec<ConnectionRecord>>(4096);
    let worker_db = state.db.clone();
    let worker_handle = tokio::spawn(async move {
        while let Some(records) = closed_rx.recv().await {
            if let Err(e) = persist_local_closed_connection_logs(worker_db.clone(), records).await {
                tracing::error!(target: "backend::master::logs", "审计日志 worker 处理失败: {:?}", e);
            }
        }
        tracing::debug!(target: "backend::master::logs", "审计日志 worker 已结束");
    });
    let mut guard = state.closed_worker_handle.lock().await;
    *guard = Some(worker_handle);
    drop(guard);

    // 运行Mihomo客户端连接
    mihomo_client
        .connect(move |data| {
            // 获取数据库和当前状态
            let db = &state.db;

            // 获取并锁定当前状态
            let mut state_lock = match state.conn_state.lock() {
                Ok(lock) => lock,
                Err(e) => {
                    tracing::error!(target: "backend::master::mihomo", "连接状态锁被污染，尝试恢复: {:?}", e);
                    e.into_inner()
                }
            };

            let current_conn_ids: std::collections::HashSet<_> = data
                .connections
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
                    target: "backend::master::logs",
                    "检测到 {} 条本地关闭连接缺少内存快照，可能影响审计日志完整性",
                    missing_closed
                );
            }

            // 使用公共函数处理连接更新
            if let Err(e) = process_connections(&data, &mut state_lock, db.clone(), agent_id.clone()) {
                tracing::error!(target: "backend::master::mihomo", "处理连接数据失败: {}", e);
                return Err(format!("处理连接数据失败: {}", e));
            }

            if !closed_records.is_empty() {
                match closed_tx.try_send(closed_records) {
                    Ok(()) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(records)) => {
                        tracing::error!(target: "backend::master::logs", "审计日志队列已满，丢弃 {} 条关闭连接记录", records.len());
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        tracing::error!(target: "backend::master::logs", "审计日志 worker 已关闭");
                    }
                }
            }

            Ok(())
        })
        .await
}

async fn persist_local_closed_connection_logs(
    db: Arc<Database>,
    closed_records: Vec<ConnectionRecord>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if closed_records.is_empty() {
        return Ok(());
    }

    let logs: Vec<crate::common::ConnectionLog> = closed_records
        .iter()
        .map(|record| {
            let mut log = ConnectionLog::from(record);
            log.synced = Some(1);
            log
        })
        .collect();

    db.batch_insert_connection_logs(&logs).await.map_err(|e| {
        tracing::error!(
            target: "backend::master::logs",
            "批量保存本地 connection_closed 失败: {:?}",
            e
        );
        Box::new(e) as Box<dyn Error + Send + Sync>
    })?;

    for log in logs {
        publish_log_event(LogStreamEvent::ConnectionClosed {
            timestamp: Utc::now().to_rfc3339(),
            connection: Box::new(log),
        });
    }
    Ok(())
}
