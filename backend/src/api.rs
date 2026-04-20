use crate::common::{ConnectionLog, GlobalData, LogStreamEvent, SyncPackage};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::error::Error;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::broadcast::{self, Receiver};
use tokio_tungstenite::connect_async;
use warp::http::header::{AUTHORIZATION, CONTENT_TYPE};
use warp::http::Method;
use warp::http::StatusCode;
use warp::reply::Json;
use warp::ws::{Message, WebSocket};
use warp::Filter;

static LOG_EVENT_BROADCASTER: OnceLock<broadcast::Sender<LogStreamEvent>> = OnceLock::new();

fn ensure_log_event_broadcaster() -> &'static broadcast::Sender<LogStreamEvent> {
    LOG_EVENT_BROADCASTER.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(10_000);
        tx
    })
}

pub fn publish_log_event(event: LogStreamEvent) {
    if let Err(e) = ensure_log_event_broadcaster().send(event) {
        tracing::debug!(target: "backend::master::logs", "无活跃 WebSocket 接收者，日志事件被丢弃: {:?}", e);
    }
}

fn subscribe_log_events() -> Receiver<LogStreamEvent> {
    ensure_log_event_broadcaster().subscribe()
}

// Mihomo API客户端
#[derive(Clone)]
pub struct MihomoClient {
    host: String,
    port: u16,
    token: String,
}

impl MihomoClient {
    pub fn new(host: String, port: u16, token: String) -> Self {
        Self { host, port, token }
    }

    // 连接到 Mihomo WebSocket API 并处理数据，回调返回 Result 以支持错误终止
    pub async fn connect<F>(&self, mut callback: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(GlobalData) -> Result<(), String> + Send + 'static,
    {
        let encoded_token = urlencoding::encode(&self.token);
        let ws_uri = format!(
            "ws://{}:{}/connections?token={}",
            self.host, self.port, encoded_token
        );

        tracing::info!(target: "backend::mihomo", "连接到Mihomo API: {}:{}...", self.host, self.port);

        loop {
            match connect_async(&ws_uri).await {
                Ok((ws_stream, _)) => {
                    tracing::info!(target: "backend::mihomo", "已连接到Mihomo API");
                    let (_, mut read) = ws_stream.split();

                    loop {
                        let timeout_result =
                            tokio::time::timeout(Duration::from_secs(60), read.next()).await;
                        let Ok(next) = timeout_result else {
                            tracing::warn!(target: "backend::mihomo", "WebSocket 读取超时，重新连接...");
                            break;
                        };
                        let Some(Ok(message)) = next else {
                            match next {
                                Some(Err(e)) => {
                                    tracing::warn!(target: "backend::mihomo", "WebSocket错误，重新连接: {:?}", e)
                                }
                                _ => {
                                    tracing::info!(target: "backend::mihomo", "WebSocket连接已关闭，重新连接中...")
                                }
                            }
                            break;
                        };
                        if !message.is_text() {
                            continue;
                        }
                        let text = match message.into_text() {
                            Ok(t) => t,
                            Err(e) => {
                                tracing::warn!(target: "backend::mihomo", "消息转换失败: {:?}, 重新连接...", e);
                                break;
                            }
                        };
                        let data = match serde_json::from_str::<GlobalData>(&text) {
                            Ok(d) => d,
                            Err(e) => {
                                tracing::warn!(target: "backend::mihomo", "解析消息失败: {}\n原始数据: {}", e, text);
                                continue;
                            }
                        };
                        if let Err(e) = callback(data) {
                            tracing::error!(target: "backend::mihomo", "处理数据出错: {}", e);
                            return Err(e.into());
                        }
                    }
                    // 内层循环结束（连接断开/超时/错误），统一退避后再重连
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    tracing::warn!(target: "backend::mihomo", "连接错误: {:?}, 重试中...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

// 主节点API客户端
pub struct MasterClient {
    client: Client,
    master_url: String,
    master_token: Option<String>,
}

impl MasterClient {
    pub fn new(master_url: String, master_token: Option<String>) -> Result<Self, reqwest::Error> {
        let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

        Ok(Self {
            client,
            master_url,
            master_token,
        })
    }

    pub async fn is_online(&self) -> Result<bool, reqwest::Error> {
        let health_url = format!("{}/api/v1/health", self.master_url);
        let response = self.client.get(&health_url).send().await?;
        Ok(response.status().is_success())
    }

    fn apply_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.master_token {
            Some(token) => request.header("Authorization", format!("Bearer {}", token)),
            None => request,
        }
    }

    pub async fn sync_data(
        &self,
        package: &SyncPackage,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let sync_url = format!("{}/api/v1/sync", self.master_url);
        let response = self
            .apply_auth(self.client.post(&sync_url))
            .json(package)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("同步失败: {} - {}", status, text).into());
        }

        Ok(())
    }

    pub async fn sync_connection_logs(
        &self,
        agent_id: &str,
        logs: &[ConnectionLog],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let url = format!("{}/api/v1/logs/connections/sync", self.master_url);
        let response = self
            .apply_auth(self.client.post(&url))
            .json(&json!({
                "agent_id": agent_id,
                "logs": logs
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("同步 connection_logs 失败: {} - {}", status, text).into());
        }

        Ok(())
    }

    pub async fn report_connection_closed(
        &self,
        log: &ConnectionLog,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let url = format!("{}/api/v1/logs/connection-closed", self.master_url);
        let response = self
            .apply_auth(self.client.post(&url))
            .json(log)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("实时上报 connection_closed 失败: {} - {}", status, text).into());
        }

        Ok(())
    }
}

// API 响应结构
#[derive(Serialize)]
struct ApiResponse<T: Serialize> {
    status: String,
    data: Option<T>,
    message: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    // 创建成功响应
    fn success(data: T) -> Json {
        warp::reply::json(&ApiResponse {
            status: "success".to_string(),
            data: Some(data),
            message: None,
        })
    }

    // 创建错误响应
    fn error(message: &str) -> Json {
        warp::reply::json(&ApiResponse::<()> {
            status: "error".to_string(),
            data: None,
            message: Some(message.to_string()),
        })
    }
}

// 统一的API响应类型
type ApiResult = Result<Json, warp::Rejection>;

// API错误类型

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,          // 未授权
    BadRequest(String),    // 请求参数错误
    DatabaseError(String), // 数据库错误
    NotFound,              // 资源不存在
    #[allow(dead_code)]
    InternalError(String), // 内部服务器错误
}

impl warp::reject::Reject for ApiError {}

#[derive(Deserialize, Debug, Clone)]
pub struct StatsQuery {
    r#type: String,
    group_by: Option<String>,
    interval: Option<String>,
    metric: Option<String>,
    from: Option<String>,
    to: Option<String>,
    agent_id: Option<String>,
    network: Option<String>,
    rule: Option<String>,
    process: Option<String>,
    destination: Option<String>,
    source: Option<String>,
    host: Option<String>,
    chains: Option<String>,
    destination_port: Option<String>,
    exclude_rule: Option<String>,
    limit: Option<u32>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConnectionsQuery {
    agent_id: Option<String>,
    network: Option<String>,
    rule: Option<String>,
    process: Option<String>,
    source: Option<String>,
    destination: Option<String>,
    host: Option<String>,
    chains: Option<String>,
    destination_port: Option<String>,
    exclude_rule: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AgentQuery {
    exclude_rule: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FilterOptionsQuery {
    filter_type: String,
    query: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
struct WsAuthQuery {
    token: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConnectionLogsQuery {
    agent_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    source: Option<String>,
    host: Option<String>,
    rule: Option<String>,
    network: Option<String>,
    keyword: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Serialize, Debug, Clone)]
struct ConnectionLogPage {
    total: i64,
    items: Vec<ConnectionLog>,
}

#[derive(Deserialize, Debug, Clone)]
struct SyncConnectionLogsRequest {
    agent_id: String,
    logs: Vec<ConnectionLog>,
}

const MAX_SYNC_LOGS_BATCH: usize = 10_000;

fn normalize_sync_connection_logs_request(
    body: SyncConnectionLogsRequest,
) -> Result<(String, Vec<ConnectionLog>), ApiError> {
    let agent_id = body.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err(ApiError::BadRequest("agent_id is required".to_string()));
    }

    if body.logs.len() > MAX_SYNC_LOGS_BATCH {
        return Err(ApiError::BadRequest(format!(
            "logs batch exceeds maximum of {}",
            MAX_SYNC_LOGS_BATCH
        )));
    }

    if body.logs.iter().any(|log| log.id.trim().is_empty()) {
        return Err(ApiError::BadRequest(
            "each log must have a non-empty id".to_string(),
        ));
    }

    if body.logs.iter().any(|log| {
        let log_agent = log.agent_id.trim();
        !log_agent.is_empty() && log_agent != agent_id
    }) {
        return Err(ApiError::BadRequest(
            "agent_id mismatch between request and logs".to_string(),
        ));
    }

    let mut logs = body.logs;
    // 通过验证后才修改数据：强制覆盖 agent_id 并标记为已同步
    for log in &mut logs {
        log.agent_id = agent_id.clone();
        log.synced = Some(1);
    }

    Ok((agent_id, logs))
}

// 主节点API服务器
pub struct MasterServer {
    database: Arc<crate::db::Database>,
    api_token: Option<String>,
}

impl MasterServer {
    pub fn new(database: Arc<crate::db::Database>, api_token: Option<String>) -> Self {
        let _ = ensure_log_event_broadcaster();
        Self {
            database,
            api_token,
        }
    }

    // 构建API认证过滤器
    fn with_auth(&self) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
        let token = self.api_token.clone();

        warp::header::optional::<String>("authorization")
            .and_then(move |auth: Option<String>| {
                let token = token.clone();
                async move {
                    // 如果未设置令牌，则无需验证
                    let Some(expected) = token else {
                        return Ok(());
                    };

                    match auth {
                        Some(auth_header) if auth_header.starts_with("Bearer ") => {
                            let provided_token = auth_header[7..].to_string();
                            if provided_token == expected {
                                Ok(())
                            } else {
                                Err(warp::reject::custom(ApiError::Unauthorized))
                            }
                        }
                        _ => Err(warp::reject::custom(ApiError::Unauthorized)),
                    }
                }
            })
            .untuple_one()
    }

    fn with_ws_auth(&self) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone {
        let token = self.api_token.clone();

        warp::header::optional::<String>("authorization")
            .and(
                warp::query::<WsAuthQuery>()
                    .or(warp::any().map(|| WsAuthQuery { token: None }))
                    .unify(),
            )
            .and_then(move |auth: Option<String>, query: WsAuthQuery| {
                let token = token.clone();
                async move {
                    let Some(expected) = token else {
                        return Ok(());
                    };

                    let auth_ok = auth
                        .as_deref()
                        .and_then(|raw| raw.strip_prefix("Bearer "))
                        .is_some_and(|provided| provided == expected);
                    let query_ok = query
                        .token
                        .as_deref()
                        .is_some_and(|provided| provided == expected);

                    if auth_ok || query_ok {
                        Ok(())
                    } else {
                        Err(warp::reject::custom(ApiError::Unauthorized))
                    }
                }
            })
            .untuple_one()
    }

    // 构建数据库访问过滤器
    fn with_db(
        &self,
    ) -> impl Filter<Extract = (Arc<crate::db::Database>,), Error = Infallible> + Clone {
        let db = self.database.clone();
        warp::any().map(move || db.clone())
    }

    // 启动API服务器
    pub async fn start(&self, host: &str, port: u16) -> Result<(), Box<dyn Error>> {
        // 构建所有API路由
        let routes = self.build_routes();

        let addr_str = if host.contains(':') {
            format!("[{}]:{}", host, port)
        } else {
            format!("{}:{}", host, port)
        };
        let socket_addr: std::net::SocketAddr =
            addr_str.parse().map_err(|e| format!("无效的地址: {}", e))?;

        // 预绑定以优雅捕获地址冲突或权限错误，避免 panic
        let std_listener =
            std::net::TcpListener::bind(socket_addr).map_err(|e| format!("绑定失败: {}", e))?;
        let bound_addr = std_listener
            .local_addr()
            .map_err(|e| format!("获取本地地址失败: {}", e))?;
        drop(std_listener);

        tracing::info!(target: "backend::master::api", "启动主节点API服务器在 http://{}...", bound_addr);
        warp::serve(routes).run(bound_addr).await;

        Ok(())
    }

    // 构建所有API路由
    fn build_routes(&self) -> impl Filter<Extract = impl warp::Reply, Error = Infallible> + Clone {
        // 健康检查路由
        let health_route = warp::path!("api" / "v1" / "health")
            .and(warp::get())
            .map(|| "OK");

        // 获取服务器时区
        let timezone_route = warp::path!("api" / "v1" / "timezone")
            .and(warp::get())
            .and_then(|| async move {
                let offset = chrono::Local::now().offset().local_minus_utc() / 60;
                let sign = if offset >= 0 { "+" } else { "-" };
                let abs_offset = offset.abs();
                let hours = abs_offset / 60;
                let minutes = abs_offset % 60;
                let tz_name = format!("UTC{sign}{:02}:{:02}", hours, minutes);
                Ok::<_, warp::reject::Rejection>(warp::reply::json(&json!({
                    "timezone": tz_name,
                    "offset_minutes": offset,
                })))
            });

        // 同步路由
        let sync_route = warp::path!("api" / "v1" / "sync")
            .and(warp::post())
            .and(self.with_auth())
            .and(warp::body::json())
            .and(self.with_db())
            .and_then(
                |sync_package: SyncPackage, db: Arc<crate::db::Database>| async move {
                    handle_sync(sync_package, db).await
                },
            );

        // 审计日志批量同步路由
        let sync_connection_logs_route =
            warp::path!("api" / "v1" / "logs" / "connections" / "sync")
                .and(warp::post())
                .and(self.with_auth())
                .and(warp::body::json())
                .and(self.with_db())
                .and_then(
                    |body: SyncConnectionLogsRequest, db: Arc<crate::db::Database>| async move {
                        match normalize_sync_connection_logs_request(body) {
                            Ok((agent_id, logs)) => {
                                handle_sync_connection_logs(agent_id, logs, db).await
                            }
                            Err(e) => Err(warp::reject::custom(e)),
                        }
                    },
                );

        // 连接关闭实时上报路由
        let connection_closed_route = warp::path!("api" / "v1" / "logs" / "connection-closed")
            .and(warp::post())
            .and(self.with_auth())
            .and(warp::body::json())
            .and(self.with_db())
            .and_then(
                |connection: ConnectionLog, db: Arc<crate::db::Database>| async move {
                    handle_connection_closed(connection, db).await
                },
            );

        // 统计路由 - 统一入口
        let stats_route = warp::path!("api" / "v1" / "stats")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<StatsQuery>())
            .and(self.with_db())
            .and_then(
                |query: StatsQuery, db: Arc<crate::db::Database>| async move {
                    handle_stats(query, db).await
                },
            );

        // 连接查询路由
        let connections_route = warp::path!("api" / "v1" / "connections")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<ConnectionsQuery>())
            .and(self.with_db())
            .and_then(
                |query: ConnectionsQuery, db: Arc<crate::db::Database>| async move {
                    handle_connections(query, db).await
                },
            );

        // 历史审计日志查询路由
        let connection_logs_route = warp::path!("api" / "v1" / "logs" / "connections")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<ConnectionLogsQuery>())
            .and(self.with_db())
            .and_then(
                |query: ConnectionLogsQuery, db: Arc<crate::db::Database>| async move {
                    handle_connection_logs(query, db).await
                },
            );

        // 实时日志 WebSocket 路由
        let ws_logs_route = warp::path!("ws" / "logs")
            .and(self.with_ws_auth())
            .and(warp::ws())
            .map(|ws: warp::ws::Ws| ws.on_upgrade(handle_logs_ws));

        // 代理节点查询路由
        let agents_route = warp::path!("api" / "v1" / "agents")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<AgentQuery>())
            .and(self.with_db())
            .and_then(
                |query: AgentQuery, db: Arc<crate::db::Database>| async move {
                    handle_agents(query, db).await
                },
            );

        // 代理节点状态查询路由
        let agent_status_route = warp::path!("api" / "v1" / "agents" / String / "status")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<AgentQuery>())
            .and(self.with_db())
            .and_then(
                |agent_id: String, query: AgentQuery, db: Arc<crate::db::Database>| async move {
                    handle_agent_status(agent_id, query, db).await
                },
            );

        // 筛选器选项查询路由
        let filter_options_route = warp::path!("api" / "v1" / "filter-options")
            .and(warp::get())
            .and(self.with_auth())
            .and(warp::query::<FilterOptionsQuery>())
            .and(self.with_db())
            .and_then(
                |query: FilterOptionsQuery, db: Arc<crate::db::Database>| async move {
                    handle_filter_options(query, db).await
                },
            );

        // 配置 CORS：设置了 API token 时允许任意源，未设置时仅允许基本跨域
        let cors = if self.api_token.is_some() {
            warp::cors()
                .allow_any_origin()
                .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(vec![CONTENT_TYPE, AUTHORIZATION])
                .allow_credentials(false)
        } else {
            warp::cors()
                .allow_methods(vec![Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(vec![CONTENT_TYPE, AUTHORIZATION])
                .allow_credentials(false)
        };

        // 合并所有路由并添加 CORS 和错误处理
        health_route
            .or(timezone_route)
            .or(sync_route)
            .or(sync_connection_logs_route)
            .or(connection_closed_route)
            .or(stats_route)
            .or(connections_route)
            .or(connection_logs_route)
            .or(agents_route)
            .or(agent_status_route)
            .or(filter_options_route)
            .or(ws_logs_route)
            .with(cors)
            .recover(handle_rejection)
    }
}

// 处理统计请求的统一入口
async fn handle_stats(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    // 根据统计类型分发到不同的处理函数
    match query.r#type.as_str() {
        "summary" => handle_stats_summary(query, db).await,
        "group" => handle_stats_group(query, db).await,
        "timeseries" => handle_stats_timeseries(query, db).await,
        _ => Err(warp::reject::custom(ApiError::BadRequest(format!(
            "不支持的统计类型: {}",
            query.r#type
        )))),
    }
}

// 处理汇总统计请求
async fn handle_stats_summary(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    // 构建基础SQL和参数
    let (count_sql, count_params) =
        build_base_filter("SELECT COUNT(*) as count FROM connection_logs", &query);

    let (traffic_sql, traffic_params) = build_base_filter(
        "SELECT SUM(download) as download, SUM(upload) as upload FROM connection_logs",
        &query,
    );

    // 执行查询
    let count_result = db.execute_count_query(&count_sql, &count_params).await;
    let traffic_result = db
        .execute_traffic_query(&traffic_sql, &traffic_params)
        .await;

    match (count_result, traffic_result) {
        (Ok(count), Ok((download, upload))) => Ok(ApiResponse::success(json!({
            "count": count,
            "download": download,
            "upload": upload,
            "total": download + upload
        }))),
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(target: "backend::master::api", "统计查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "统计查询失败: {}",
                e
            ))))
        }
    }
}

async fn handle_stats_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    let Some(group_field) = &query.group_by else {
        return Err(warp::reject::custom(ApiError::BadRequest(
            "分组统计必须指定 group_by 参数".to_string(),
        )));
    };
    let group_field = group_field.as_str();

    match group_field {
        "host" => handle_host_group(query.clone(), db).await,
        "chains" => handle_chains_group(query.clone(), db).await,
        "rule" => handle_rule_group(query.clone(), db).await,
        "destination" => handle_destination_group(query.clone(), db).await,
        "source" => handle_source_group(query.clone(), db).await,
        "network" | "process" | "destination_port" => {
            handle_standard_group(query.clone(), db, group_field).await
        }
        _ => Err(warp::reject::custom(ApiError::BadRequest(format!(
            "不支持的分组字段: {}",
            group_field
        )))),
    }
}

// 处理主机分组
async fn handle_host_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    // 构建主机查询SQL
    let base_sql = r#"
        SELECT 
            COALESCE(NULLIF(host, ''), destination_ip) as host_display,
                COUNT(*) as count, 
                SUM(download) as download, 
                SUM(upload) as upload 
        FROM connection_logs
    "#;

    let (mut sql, params) = build_base_filter(base_sql, &query);

    // 添加分组和排序
    sql.push_str(" GROUP BY host_display");

    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    // 执行查询
    match db.execute_host_group_query(&sql, &params).await {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "主机分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "主机分组查询失败: {}",
                e
            ))))
        }
    }
}

// 处理代理链路分组
async fn handle_chains_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    // 构建链路查询SQL
    let base_sql = r#"
        SELECT 
            chains,
            COUNT(*) as count, 
            SUM(download) as download, 
            SUM(upload) as upload 
        FROM connection_logs
    "#;

    let (mut sql, params) = build_base_filter(base_sql, &query);

    // 添加分组和排序
    sql.push_str(" GROUP BY chains");

    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    // 执行查询
    match db.execute_group_query(&sql, &params, Some("chains")).await {
        Ok(results) => {
            // 处理链路结果，提取最后节点
            match db
                .process_chains_results(
                    results,
                    query.sort_by.as_deref(),
                    query.sort_order.as_deref(),
                    query.limit,
                )
                .await
            {
                Ok(processed) => Ok(ApiResponse::success(processed)),
                Err(e) => {
                    tracing::error!(target: "backend::master::api", "链路分组结果处理错误: {}", e);
                    Err(warp::reject::custom(ApiError::DatabaseError(format!(
                        "链路分组结果处理失败: {}",
                        e
                    ))))
                }
            }
        }
        Err(e) => {
            tracing::error!(target: "backend::master::api", "链路分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "链路分组查询失败: {}",
                e
            ))))
        }
    }
}

// 处理规则分组（按 rule 字段归类）
async fn handle_rule_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    let base_sql = r#"
        SELECT 
            rule,
            COUNT(*) as count, 
            SUM(download) as download, 
            SUM(upload) as upload 
        FROM connection_logs
    "#;

    let (mut sql, params) = build_base_filter(base_sql, &query);
    sql.push_str(" GROUP BY rule");
    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    match db.execute_group_query(&sql, &params, Some("rule")).await {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "规则分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "规则分组查询失败: {}",
                e
            ))))
        }
    }
}

// 处理目标地址分组
async fn handle_destination_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    let base_sql = r#"
        SELECT 
            COALESCE(NULLIF(host, ''), destination_ip) as host_display,
            destination_ip,
            COUNT(*) as count, 
            SUM(download) as download, 
            SUM(upload) as upload 
        FROM connection_logs
    "#;

    let (mut sql, params) = build_base_filter(base_sql, &query);
    sql.push_str(" GROUP BY host_display");
    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    match db.execute_destination_group_query(&sql, &params).await {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "目标地址分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "目标地址分组查询失败: {}",
                e
            ))))
        }
    }
}

// 处理源地址分组
async fn handle_source_group(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    let base_sql = r#"
        SELECT 
            source_ip as host_display,
            source_ip,
            COUNT(*) as count, 
            SUM(download) as download, 
            SUM(upload) as upload 
        FROM connection_logs
    "#;

    let (mut sql, params) = build_base_filter(base_sql, &query);
    sql.push_str(" GROUP BY source_ip");
    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    match db.execute_source_group_query(&sql, &params).await {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "源地址分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "源地址分组查询失败: {}",
                e
            ))))
        }
    }
}

// 处理标准分组
async fn handle_standard_group(
    query: StatsQuery,
    db: Arc<crate::db::Database>,
    group_field: &str,
) -> ApiResult {
    let (column_name, sql_expression) = match group_field {
        "network" => ("network", "network"),
        "process" => (
            "process",
            "CASE WHEN process = '' OR process IS NULL THEN '进程为空' ELSE process END as process",
        ),
        "destination_port" => ("destination_port", "destination_port"),
        _ => {
            return Err(warp::reject::custom(ApiError::BadRequest(format!(
                "不支持的标准分组字段: {}",
                group_field
            ))));
        }
    };

    // 构建查询SQL
    let base_sql = format!(
        "SELECT {}, COUNT(*) as count, SUM(download) as download, SUM(upload) as upload FROM connection_logs",
        sql_expression
    );

    let (mut sql, params) = build_base_filter(&base_sql, &query);

    // 添加分组和排序
    sql.push_str(&format!(" GROUP BY {}", column_name));

    if let Err(e) = add_sort_clause(&mut sql, &query) {
        return Err(warp::reject::custom(e));
    }
    add_limit_clause(&mut sql, &query);

    // 执行查询
    match db
        .execute_group_query(&sql, &params, Some(column_name))
        .await
    {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "标准分组查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "标准分组查询失败: {}",
                e
            ))))
        }
    }
}

fn add_sort_clause(sql: &mut String, query: &StatsQuery) -> Result<(), ApiError> {
    let sort_field = query.sort_by.as_deref().unwrap_or("count");
    let sort_field = match sort_field {
        "count" => "count",
        "download" => "download",
        "upload" => "upload",
        "total" => "(SUM(download) + SUM(upload))",
        _ => {
            return Err(ApiError::BadRequest(format!(
                "无效的排序字段: {}",
                sort_field
            )))
        }
    };
    let sort_order = if query.sort_order.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    sql.push_str(&format!(" ORDER BY {} {}", sort_field, sort_order));
    Ok(())
}

// 添加限制子句
fn add_limit_clause(sql: &mut String, query: &StatsQuery) {
    if let Some(limit) = query.limit {
        sql.push_str(&format!(" LIMIT {}", limit));
    }
}

async fn handle_stats_timeseries(query: StatsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    let Some(_from) = &query.from else {
        return Err(warp::reject::custom(ApiError::BadRequest(
            "时间序列查询必须指定 from 参数".to_string(),
        )));
    };
    let Some(_to) = &query.to else {
        return Err(warp::reject::custom(ApiError::BadRequest(
            "时间序列查询必须指定 to 参数".to_string(),
        )));
    };

    // 获取时间间隔和指标
    let interval = query.interval.as_deref().unwrap_or("day");
    let metric = query.metric.as_deref().unwrap_or("connections");

    // 根据选择的时间间隔确定SQL时间格式
    let time_format = match interval {
        "minute" => "%Y-%m-%d %H:%M:00",
        "hour" => "%Y-%m-%d %H:00:00",
        "day" => "%Y-%m-%d",
        "week" => "%Y-%W",
        "month" => "%Y-%m",
        _ => {
            return Err(warp::reject::custom(ApiError::BadRequest(format!(
                "不支持的时间间隔: {}",
                interval
            ))))
        }
    };

    // 根据选择的指标确定要计算的字段
    let (select_expr, group_expr) = match metric {
        "connections" => ("COUNT(*) as value", ""),
        "download" => ("SUM(download) as value", ""),
        "upload" => ("SUM(upload) as value", ""),
        "total" => ("SUM(download + upload) as value", ""),
        _ => {
            return Err(warp::reject::custom(ApiError::BadRequest(format!(
                "不支持的指标: {}",
                metric
            ))))
        }
    };

    // 构建查询SQL
    let base_sql = format!(
        "SELECT strftime('{}', datetime(end)) as time_point, {} FROM connection_logs",
        time_format, select_expr
    );

    let (mut sql, params) = build_base_filter(&base_sql, &query);
    // build_base_filter 已保证 from/to 时间范围过滤

    // 添加分组
    sql.push_str(&format!(" GROUP BY time_point{}", group_expr));
    sql.push_str(" ORDER BY time_point ASC");

    // 执行查询
    match db.execute_timeseries_query(&sql, &params).await {
        Ok(results) => Ok(ApiResponse::success(results)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "时间序列查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "时间序列查询失败: {}",
                e
            ))))
        }
    }
}

// 构建基础SQL查询过滤条件
fn build_base_filter(base_sql: &str, query: &StatsQuery) -> (String, Vec<String>) {
    let mut sql = format!("{} WHERE 1=1", base_sql);
    let mut params: Vec<String> = Vec::new();

    // 添加时间范围过滤（基于 connection_logs 的 end 字段，使用 Master 本地时区）
    if let Some(from) = &query.from {
        sql.push_str(" AND datetime(end) >= datetime(?)");
        params.push(from.clone());
    }

    if let Some(to) = &query.to {
        sql.push_str(" AND datetime(end) <= datetime(?)");
        params.push(to.clone());
    }

    // 添加其他筛选条件
    crate::db::add_filter_condition(&mut sql, &mut params, "agent_id", &query.agent_id, "=");
    crate::db::add_filter_condition(&mut sql, &mut params, "network", &query.network, "=");
    crate::db::add_filter_condition(&mut sql, &mut params, "rule", &query.rule, "=");
    if let Some(process) = &query.process {
        if process.is_empty() || process == "进程为空" {
            sql.push_str(" AND (process = '' OR process IS NULL)");
        } else {
            crate::db::add_filter_condition_with_wildcards(
                &mut sql,
                &mut params,
                "process",
                &Some(process.clone()),
                "LIKE",
                true,
            );
        }
    }
    crate::db::add_filter_condition(&mut sql, &mut params, "source_ip", &query.source, "=");
    if let Some(destination) = &query.destination {
        sql.push_str(" AND (destination_ip LIKE ? ESCAPE '\\' OR host LIKE ? ESCAPE '\\')");
        params.push(format!(
            "%{}%",
            destination.replace('%', "\\%").replace('_', "\\_")
        ));
        params.push(format!(
            "%{}%",
            destination.replace('%', "\\%").replace('_', "\\_")
        ));
    }
    crate::db::add_filter_condition_with_wildcards(
        &mut sql,
        &mut params,
        "host",
        &query.host,
        "LIKE",
        true,
    );
    crate::db::add_filter_condition_with_wildcards(
        &mut sql,
        &mut params,
        "chains",
        &query.chains,
        "LIKE",
        true,
    );
    crate::db::add_filter_condition(
        &mut sql,
        &mut params,
        "destination_port",
        &query.destination_port,
        "=",
    );
    crate::db::add_filter_condition(&mut sql, &mut params, "rule", &query.exclude_rule, "!=");

    (sql, params)
}

// 处理同步请求
async fn handle_sync(sync_package: SyncPackage, db: Arc<crate::db::Database>) -> ApiResult {
    match db
        .replace_connections_for_agent(&sync_package.agent_id, &sync_package.connections)
        .await
    {
        Ok(_) => {
            tracing::info!(
                target: "backend::master::sync",
                "成功从节点 {} 同步了 {} 条活跃连接",
                sync_package.agent_id,
                sync_package.connections.len()
            );
            Ok(ApiResponse::success(json!({
                "message": "数据同步成功",
                "count": sync_package.connections.len(),
            })))
        }
        Err(e) => {
            tracing::error!(target: "backend::master::sync", "同步出错: {:?}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "数据同步失败: {}",
                e
            ))))
        }
    }
}

async fn handle_sync_connection_logs(
    agent_id: String,
    logs: Vec<ConnectionLog>,
    db: Arc<crate::db::Database>,
) -> ApiResult {
    match db.batch_insert_connection_logs(&logs).await {
        Ok(_) => {
            tracing::info!(
                target: "backend::master::sync",
                "成功从节点 {} 同步 {} 条 connection_logs",
                agent_id,
                logs.len()
            );
            Ok(ApiResponse::success(json!({
                "message": "connection_logs 同步成功",
                "count": logs.len(),
            })))
        }
        Err(e) => {
            tracing::error!(target: "backend::master::sync", "connection_logs 同步出错: {:?}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "connection_logs 同步失败: {}",
                e
            ))))
        }
    }
}

async fn handle_connection_closed(
    mut connection: ConnectionLog,
    db: Arc<crate::db::Database>,
) -> ApiResult {
    let agent_id = connection.agent_id.trim();
    if agent_id.is_empty() {
        return Err(warp::reject::custom(ApiError::BadRequest(
            "agent_id is required".to_string(),
        )));
    }
    if connection.id.trim().is_empty() {
        return Err(warp::reject::custom(ApiError::BadRequest(
            "connection id is required".to_string(),
        )));
    }
    connection.agent_id = agent_id.to_string();
    connection.synced = Some(1);
    if let Err(e) = db.insert_connection_log(&connection).await {
        tracing::error!(target: "backend::master::logs", "保存 connection_closed 失败: {:?}", e);
        return Err(warp::reject::custom(ApiError::DatabaseError(format!(
            "保存 connection_closed 失败: {}",
            e
        ))));
    }

    publish_log_event(LogStreamEvent::ConnectionClosed {
        timestamp: Utc::now().to_rfc3339(),
        connection: Box::new(connection.clone()),
    });

    Ok(ApiResponse::success(json!({ "message": "ok" })))
}

async fn handle_connection_logs(
    query: ConnectionLogsQuery,
    db: Arc<crate::db::Database>,
) -> ApiResult {
    let limit = query.limit.unwrap_or(20).clamp(1, 100) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    match db
        .query_connection_logs(
            query.agent_id.as_deref(),
            query.from.as_deref(),
            query.to.as_deref(),
            query.source.as_deref(),
            query.host.as_deref(),
            query.rule.as_deref(),
            query.network.as_deref(),
            query.keyword.as_deref(),
            query.sort_by.as_deref(),
            query.sort_order.as_deref(),
            limit,
            offset,
        )
        .await
    {
        Ok((items, total)) => Ok(ApiResponse::success(ConnectionLogPage { total, items })),
        Err(e) => {
            tracing::error!(target: "backend::master::logs", "查询 connection_logs 失败: {:?}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "查询 connection_logs 失败: {}",
                e
            ))))
        }
    }
}

async fn handle_logs_ws(ws: WebSocket) {
    let (mut sender, mut receiver) = ws.split();
    let mut rx = subscribe_log_events();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(Ok(msg)) = incoming else {
                    if let Some(Err(e)) = incoming {
                        tracing::warn!(target: "backend::master::logs", "ws/logs 接收错误: {:?}", e);
                    }
                    break;
                };
                if msg.is_close() {
                    break;
                }
                tracing::debug!(target: "backend::master::api", "ws/logs 收到非关闭消息，已忽略");
            }
            event = rx.recv() => {
                match event {
                    Ok(event) => {
                        match serde_json::to_string(&event) {
                            Ok(payload) => {
                                if let Err(e) = sender.send(Message::text(payload)).await {
                                    tracing::warn!(target: "backend::master::logs", "ws/logs 发送失败: {:?}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!(target: "backend::master::logs", "日志事件序列化失败: {:?}", e);
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(target: "backend::master::logs", "ws/logs 客户端落后，丢失 {} 条事件，断开连接", skipped);
                        let gap_notice = serde_json::json!({
                            "event_type": "gap",
                            "skipped": skipped,
                            "message": format!("客户端处理速度过慢，已丢失 {} 条事件", skipped)
                        });
                        match serde_json::to_string(&gap_notice) {
                            Ok(payload) => {
                                if let Err(e) = sender.send(Message::text(payload)).await {
                                    tracing::warn!(target: "backend::master::logs", "ws/logs 发送 gap 通知失败，断开客户端: {:?}", e);
                                }
                            }
                            Err(e) => tracing::error!(target: "backend::master::logs", "gap 通知序列化失败: {:?}", e),
                        }
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

// 处理连接查询请求
async fn handle_connections(query: ConnectionsQuery, db: Arc<crate::db::Database>) -> ApiResult {
    // 构建基础SQL
    let base_sql = r#"
        SELECT
            id, conn_download as download, conn_upload as upload,
            last_updated, start, network,
            source_ip, destination_ip, source_port,
            destination_port, COALESCE(NULLIF(host, ''), destination_ip) as host,
            CASE WHEN process = '' OR process IS NULL THEN '进程为空' ELSE process END as process,
            process_path, special_rules,
            chains, rule, rule_payload, agent_id
        FROM connections
    "#;

    // 构建查询条件
    let (sql, params) = match build_connections_filter(base_sql, &query) {
        Ok(r) => r,
        Err(e) => return Err(warp::reject::custom(e)),
    };

    // 执行查询
    match db.execute_connections_query(&sql, &params).await {
        Ok(connections) => Ok(ApiResponse::success(connections)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "连接查询错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "连接查询失败: {}",
                e
            ))))
        }
    }
}

// 构建连接查询条件
fn build_connections_filter(
    base_sql: &str,
    query: &ConnectionsQuery,
) -> Result<(String, Vec<String>), ApiError> {
    let mut sql = base_sql.to_string();
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    // 添加筛选条件
    if let Some(agent_id) = &query.agent_id {
        conditions.push("agent_id = ?".to_string());
        params.push(agent_id.clone());
    }

    if let Some(network) = &query.network {
        conditions.push("network = ?".to_string());
        params.push(network.clone());
    }

    if let Some(rule) = &query.rule {
        conditions.push("rule = ?".to_string());
        params.push(rule.clone());
    }

    if let Some(process) = &query.process {
        // 处理空进程的特殊情况
        if process.is_empty() || process == "进程为空" {
            conditions.push("(process = '' OR process IS NULL)".to_string());
        } else {
            conditions.push("process LIKE ? ESCAPE '\\'".to_string());
            params.push(format!(
                "%{}%",
                process.replace('%', "\\%").replace('_', "\\_")
            ));
        }
    }

    if let Some(source) = &query.source {
        conditions.push("source_ip = ?".to_string());
        params.push(source.clone());
    }

    if let Some(destination) = &query.destination {
        conditions
            .push("(destination_ip LIKE ? ESCAPE '\\' OR host LIKE ? ESCAPE '\\')".to_string());
        params.push(format!(
            "%{}%",
            destination.replace('%', "\\%").replace('_', "\\_")
        ));
        params.push(format!(
            "%{}%",
            destination.replace('%', "\\%").replace('_', "\\_")
        ));
    }

    if let Some(host) = &query.host {
        conditions.push("host LIKE ? ESCAPE '\\'".to_string());
        params.push(format!(
            "%{}%",
            host.replace('%', "\\%").replace('_', "\\_")
        ));
    }

    if let Some(chains) = &query.chains {
        conditions.push("chains LIKE ? ESCAPE '\\'".to_string());
        params.push(format!(
            "%{}%",
            chains.replace('%', "\\%").replace('_', "\\_")
        ));
    }

    if let Some(destination_port) = &query.destination_port {
        conditions.push("destination_port = ?".to_string());
        params.push(destination_port.clone());
    }

    if let Some(exclude_rule) = &query.exclude_rule {
        conditions.push("rule != ?".to_string());
        params.push(exclude_rule.clone());
    }

    if let Some(from) = &query.from {
        conditions.push("start >= ?".to_string());
        params.push(from.clone());
    }

    if let Some(to) = &query.to {
        conditions.push("start <= ?".to_string());
        params.push(to.clone());
    }

    // 添加WHERE子句
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    let sort_column = query.sort_by.as_deref().unwrap_or("last_updated");
    let sort_column = match sort_column {
        "download" => "conn_download",
        "upload" => "conn_upload",
        "total" => "(conn_download + conn_upload)",
        "start" => "start",
        "last_updated" => "last_updated",
        _ => {
            return Err(ApiError::BadRequest(format!(
                "无效的排序字段: {}",
                sort_column
            )))
        }
    };
    let sort_order = if query.sort_order.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    sql.push_str(&format!(" ORDER BY {} {}", sort_column, sort_order));

    // 添加分页
    if let Some(limit) = query.limit {
        sql.push_str(&format!(" LIMIT {}", limit));

        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
    } else {
        // 默认限制100条记录
        sql.push_str(" LIMIT 100");
    }

    Ok((sql, params))
}

// 处理代理节点相关请求
async fn handle_agents(query: AgentQuery, db: Arc<crate::db::Database>) -> ApiResult {
    match db.get_agents(query.exclude_rule.as_deref()).await {
        Ok(agents) => Ok(ApiResponse::success(agents)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取代理节点列表错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取代理节点列表失败: {}",
                e
            ))))
        }
    }
}

async fn handle_agent_status(
    agent_id: String,
    query: AgentQuery,
    db: Arc<crate::db::Database>,
) -> ApiResult {
    match db
        .get_agent_status(&agent_id, query.exclude_rule.as_deref())
        .await
    {
        Ok(status) => Ok(ApiResponse::success(status)),
        Err(sqlx::Error::RowNotFound) => Err(warp::reject::custom(ApiError::NotFound)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取代理节点状态错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取代理节点状态失败: {}",
                e
            ))))
        }
    }
}

// 处理筛选器选项请求
async fn handle_filter_options(
    query: FilterOptionsQuery,
    db: Arc<crate::db::Database>,
) -> ApiResult {
    // 根据筛选类型执行不同的查询
    match query.filter_type.as_str() {
        "agent_id" => get_agent_id_options(db, query.query, query.limit).await,
        "network" => get_network_options(db, query.query, query.limit).await,
        "rule" => get_rule_options(db, query.query, query.limit).await,
        "process" => get_process_options(db, query.query, query.limit).await,
        "destination" => get_destination_options(db, query.query, query.limit).await,
        "host" => get_host_options(db, query.query, query.limit).await,
        "destination_port" => get_destination_port_options(db, query.query, query.limit).await,
        _ => Err(warp::reject::custom(ApiError::BadRequest(format!(
            "不支持的筛选类型: {}",
            query.filter_type
        )))),
    }
}

// 获取代理ID选项
async fn get_agent_id_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql =
        "SELECT DISTINCT agent_id FROM connection_logs WHERE agent_id IS NOT NULL".to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        sql.push_str(" AND agent_id LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY agent_id");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取代理ID选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取代理ID选项失败: {}",
                e
            ))))
        }
    }
}

// 获取网络类型选项
async fn get_network_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql =
        "SELECT DISTINCT network FROM connection_logs WHERE network IS NOT NULL".to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        sql.push_str(" AND network LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY network");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取网络类型选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取网络类型选项失败: {}",
                e
            ))))
        }
    }
}

// 获取规则选项
async fn get_rule_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql = "SELECT DISTINCT rule FROM connection_logs WHERE rule IS NOT NULL".to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        sql.push_str(" AND rule LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY rule");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取规则选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取规则选项失败: {}",
                e
            ))))
        }
    }
}

// 获取进程选项
async fn get_process_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql = "SELECT DISTINCT CASE WHEN process = '' OR process IS NULL THEN '进程为空' ELSE process END as process FROM connection_logs".to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        if q == "进程为空" {
            sql.push_str(" WHERE (process = '' OR process IS NULL)");
        } else {
            sql.push_str(" WHERE process LIKE ?");
            params.push(format!("%{}%", q));
        }
    }

    sql.push_str(" ORDER BY process");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取进程选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取进程选项失败: {}",
                e
            ))))
        }
    }
}

// 获取目标地址选项
async fn get_destination_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql =
        "SELECT DISTINCT destination_ip FROM connection_logs WHERE destination_ip IS NOT NULL"
            .to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        sql.push_str(" AND destination_ip LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY destination_ip");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取目标地址选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取目标地址选项失败: {}",
                e
            ))))
        }
    }
}

// 获取目标端口选项
async fn get_destination_port_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql =
        "SELECT DISTINCT destination_port FROM connection_logs WHERE destination_port IS NOT NULL AND destination_port != ''"
            .to_string();
    let mut params: Vec<String> = Vec::new();

    if let Some(q) = query {
        sql.push_str(" AND destination_port LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY CAST(destination_port AS INTEGER)");

    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取目标端口选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取目标端口选项失败: {}",
                e
            ))))
        }
    }
}

// 获取主机选项
async fn get_host_options(
    db: Arc<crate::db::Database>,
    query: Option<String>,
    limit: Option<u32>,
) -> ApiResult {
    let mut sql = "SELECT DISTINCT host FROM connection_logs WHERE host IS NOT NULL AND host != ''"
        .to_string();
    let mut params: Vec<String> = Vec::new();

    // 添加查询条件
    if let Some(q) = query {
        sql.push_str(" AND host LIKE ?");
        params.push(format!("%{}%", q));
    }

    sql.push_str(" ORDER BY host");

    // 添加限制
    if let Some(lim) = limit {
        sql.push_str(&format!(" LIMIT {}", lim));
    }

    match db.execute_filter_options_query(&sql, &params).await {
        Ok(options) => Ok(ApiResponse::success(options)),
        Err(e) => {
            tracing::error!(target: "backend::master::api", "获取主机选项错误: {}", e);
            Err(warp::reject::custom(ApiError::DatabaseError(format!(
                "获取主机选项失败: {}",
                e
            ))))
        }
    }
}

// 处理请求错误
async fn handle_rejection(err: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "未找到请求的资源".to_string())
    } else if let Some(e) = err.find::<ApiError>() {
        match e {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "认证失败".to_string()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "未找到请求的资源".to_string()),
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        }
    } else if let Some(e) = err.find::<warp::reject::InvalidQuery>() {
        tracing::warn!(target: "backend::master::api", "无效查询参数: {:?}", e);
        (StatusCode::BAD_REQUEST, format!("无效查询参数: {}", e))
    } else if let Some(e) = err.find::<warp::body::BodyDeserializeError>() {
        tracing::warn!(target: "backend::master::api", "请求体解析失败: {:?}", e);
        (StatusCode::BAD_REQUEST, format!("请求体解析失败: {}", e))
    } else {
        tracing::error!(target: "backend::master::api", "未处理的拒绝: {:?}", err);
        let message = if cfg!(debug_assertions) {
            format!("未处理的拒绝: {:?}", err)
        } else {
            "内部服务器错误".to_string()
        };
        (StatusCode::INTERNAL_SERVER_ERROR, message)
    };

    Ok(warp::reply::with_status(
        ApiResponse::<()>::error(&message),
        code,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ConnectionLog;

    fn make_log(id: &str, agent_id: &str) -> ConnectionLog {
        ConnectionLog {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            source_ip: "192.168.1.2".to_string(),
            destination_ip: "8.8.8.8".to_string(),
            source_port: "12345".to_string(),
            destination_port: "443".to_string(),
            host: "example.com".to_string(),
            rule: "PROXY".to_string(),
            rule_payload: "".to_string(),
            chains: "node-a".to_string(),
            network: "tcp".to_string(),
            process: "chrome.exe".to_string(),
            process_path: "".to_string(),
            download: 0,
            upload: 0,
            start: "2026-04-16T08:00:00Z".to_string(),
            end: "2026-04-16T08:05:00Z".to_string(),
            special_rules: "".to_string(),
            synced: Some(0),
        }
    }

    #[test]
    fn normalize_sync_request_should_reject_mismatched_agent_id() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("c1", "agent-b")],
        };
        let result = normalize_sync_connection_logs_request(body);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => {
                assert!(msg.contains("mismatch"));
            }
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn normalize_sync_request_should_fill_empty_agent_id() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("c1", "")],
        };
        let (agent_id, logs) = normalize_sync_connection_logs_request(body).unwrap();
        assert_eq!(agent_id, "agent-a");
        assert_eq!(logs[0].agent_id, "agent-a");
        assert_eq!(logs[0].synced, Some(1));
    }

    #[test]
    fn normalize_sync_request_should_accept_matching_agent_id() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("c1", "agent-a")],
        };
        let (agent_id, logs) = normalize_sync_connection_logs_request(body).unwrap();
        assert_eq!(agent_id, "agent-a");
        assert_eq!(logs[0].agent_id, "agent-a");
        assert_eq!(logs[0].synced, Some(1));
    }

    #[test]
    fn normalize_sync_request_should_reject_empty_body_agent_id() {
        let body = SyncConnectionLogsRequest {
            agent_id: "   ".to_string(),
            logs: vec![make_log("c1", "")],
        };
        let result = normalize_sync_connection_logs_request(body);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => {
                assert!(msg.contains("required"));
            }
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn normalize_sync_request_should_reject_empty_log_id() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("", "agent-a")],
        };
        let result = normalize_sync_connection_logs_request(body);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => {
                assert!(msg.contains("non-empty id"));
            }
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn normalize_sync_request_should_trim_log_agent_id_before_mismatch_check() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![ConnectionLog {
                agent_id: "  agent-a  ".to_string(),
                ..make_log("c1", "")
            }],
        };
        let (agent_id, logs) = normalize_sync_connection_logs_request(body).unwrap();
        assert_eq!(agent_id, "agent-a");
        assert_eq!(logs[0].agent_id, "agent-a");
    }

    #[test]
    fn normalize_sync_request_should_reject_oversized_batch() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("c1", "agent-a"); MAX_SYNC_LOGS_BATCH + 1],
        };
        let result = normalize_sync_connection_logs_request(body);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => {
                assert!(msg.contains("exceeds maximum"));
            }
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn build_connections_filter_should_reject_invalid_sort_by() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: Some("invalid_column".to_string()),
            sort_order: None,
        };
        let result = build_connections_filter("SELECT * FROM connections", &query);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("无效的排序字段")),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn build_connections_filter_should_support_total_sort() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: Some("total".to_string()),
            sort_order: Some("desc".to_string()),
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("ORDER BY (conn_download + conn_upload) DESC"));
        assert!(params.is_empty());
    }

    #[test]
    fn build_connections_filter_should_handle_empty_process() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: Some("".to_string()),
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("(process = '' OR process IS NULL)"));
        assert!(params.is_empty());
    }

    #[test]
    fn build_connections_filter_should_handle_special_empty_process_label() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: Some("进程为空".to_string()),
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("(process = '' OR process IS NULL)"));
        assert!(params.is_empty());
    }

    #[test]
    fn build_connections_filter_should_include_chains_condition() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: Some("node-a".to_string()),
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("chains LIKE ? ESCAPE"));
        assert_eq!(params, vec!["%node-a%".to_string()]);
    }

    #[test]
    fn build_base_filter_should_handle_special_empty_process_label() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: Some("进程为空".to_string()),
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connections", &query);
        assert!(sql.contains("(process = '' OR process IS NULL)"));
        assert!(params.is_empty());
    }

    #[test]
    fn build_base_filter_should_include_chains_condition() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: Some("node-b".to_string()),
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connections", &query);
        assert!(sql.contains("chains LIKE ? ESCAPE"));
        assert!(params.contains(&"%node-b%".to_string()));
    }

    #[test]
    fn build_connections_filter_should_escape_wildcards_in_host() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: Some("100%_server".to_string()),
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("host LIKE ?"));
        assert_eq!(params, vec!["%100\\%\\_server%".to_string()]);
    }

    #[test]
    fn add_sort_clause_should_generate_sum_for_total() {
        let mut sql = "SELECT rule, SUM(download) as download, SUM(upload) as upload FROM connection_logs GROUP BY rule".to_string();
        let query = StatsQuery {
            r#type: "group".to_string(),
            group_by: Some("rule".to_string()),
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: Some("total".to_string()),
            sort_order: Some("desc".to_string()),
        };
        add_sort_clause(&mut sql, &query).unwrap();
        assert!(sql.contains("ORDER BY (SUM(download) + SUM(upload)) DESC"));
    }

    #[test]
    fn add_sort_clause_should_use_asc_for_ascending_order() {
        let mut sql = "SELECT rule, SUM(download) as download, SUM(upload) as upload FROM connection_logs GROUP BY rule".to_string();
        let query = StatsQuery {
            r#type: "group".to_string(),
            group_by: Some("rule".to_string()),
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: Some("download".to_string()),
            sort_order: Some("asc".to_string()),
        };
        add_sort_clause(&mut sql, &query).unwrap();
        assert!(sql.contains("ORDER BY download ASC"));
    }

    #[test]
    fn add_sort_clause_should_default_to_count_when_sort_by_is_none() {
        let mut sql = "SELECT * FROM connection_logs".to_string();
        let query = StatsQuery {
            r#type: "group".to_string(),
            group_by: Some("rule".to_string()),
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        add_sort_clause(&mut sql, &query).unwrap();
        assert!(sql.contains("ORDER BY count DESC"));
    }

    #[test]
    fn add_sort_clause_should_reject_invalid_sort_field() {
        let mut sql = "SELECT * FROM connections".to_string();
        let query = StatsQuery {
            r#type: "group".to_string(),
            group_by: Some("rule".to_string()),
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: Some("bad_field".to_string()),
            sort_order: None,
        };
        let result = add_sort_clause(&mut sql, &query);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::BadRequest(msg) => assert!(msg.contains("无效的排序字段")),
            _ => panic!("expected BadRequest"),
        }
    }

    #[test]
    fn build_base_filter_should_handle_exclude_rule() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: Some("DIRECT".to_string()),
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connections", &query);
        assert!(sql.contains("rule != ?"));
        assert_eq!(params, vec!["DIRECT".to_string()]);
    }

    #[test]
    fn build_connections_filter_should_handle_exclude_rule() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: Some("DIRECT".to_string()),
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("rule != ?"));
        assert_eq!(params, vec!["DIRECT".to_string()]);
    }

    #[test]
    fn build_connections_filter_should_handle_destination_port() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: Some("443".to_string()),
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("destination_port = ?"));
        assert_eq!(params, vec!["443".to_string()]);
    }

    #[test]
    fn build_base_filter_should_use_datetime_comparison_for_time_range() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: Some("2026-04-01T00:00:00Z".to_string()),
            to: Some("2026-04-18T23:59:59Z".to_string()),
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connection_logs", &query);
        assert!(
            sql.contains("datetime(end) >= datetime(?)"),
            "expected datetime comparison, got: {}",
            sql
        );
        assert!(
            sql.contains("datetime(end) <= datetime(?)"),
            "expected datetime comparison, got: {}",
            sql
        );
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_connections_filter_should_default_limit_to_100() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, _params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("LIMIT 100"));
    }

    #[test]
    fn build_base_filter_should_handle_destination_port() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: None,
            source: None,
            host: None,
            chains: None,
            destination_port: Some("8051".to_string()),
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connections", &query);
        assert!(sql.contains("destination_port = ?"));
        assert_eq!(params, vec!["8051".to_string()]);
    }

    #[test]
    fn build_connections_filter_should_escape_wildcards_in_destination() {
        let query = ConnectionsQuery {
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            source: None,
            destination: Some("100%_server".to_string()),
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            from: None,
            to: None,
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_connections_filter("SELECT * FROM connections", &query).unwrap();
        assert!(sql.contains("destination_ip LIKE ?"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], "%100\\%\\_server%");
    }

    #[test]
    fn build_base_filter_should_escape_wildcards_in_destination() {
        let query = StatsQuery {
            r#type: "summary".to_string(),
            group_by: None,
            interval: None,
            metric: None,
            from: None,
            to: None,
            agent_id: None,
            network: None,
            rule: None,
            process: None,
            destination: Some("100%_server".to_string()),
            source: None,
            host: None,
            chains: None,
            destination_port: None,
            exclude_rule: None,
            limit: None,
            sort_by: None,
            sort_order: None,
        };
        let (sql, params) = build_base_filter("SELECT * FROM connections", &query);
        assert!(sql.contains("destination_ip LIKE ?"));
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], "%100\\%\\_server%");
    }

    #[test]
    fn normalize_sync_request_should_accept_exactly_max_batch() {
        let body = SyncConnectionLogsRequest {
            agent_id: "agent-a".to_string(),
            logs: vec![make_log("c1", "agent-a"); MAX_SYNC_LOGS_BATCH],
        };
        let result = normalize_sync_connection_logs_request(body);
        assert!(
            result.is_ok(),
            "exactly MAX_SYNC_LOGS_BATCH should be accepted"
        );
    }
}
