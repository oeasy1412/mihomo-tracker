use crate::common::{ConnectionLog, ConnectionRecord};
use chrono::Utc;
use serde_json::{Number as JsonNumber, Value as JsonValue};
use sqlx::{
    query as sqlx_query, sqlite::SqliteConnectOptions, Column, Error as SqlxError, Executor, Row,
    Sqlite, SqlitePool,
};

// 查询结果类型
pub type QueryResult<T> = Result<T, SqlxError>;
pub type JsonResult = QueryResult<Vec<JsonValue>>;

// 数据库连接池
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    // 初始化数据库
    pub async fn new(database_path: &str) -> Result<Self, SqlxError> {
        let options = SqliteConnectOptions::new()
            .filename(database_path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;

        // 创建连接表
        Self::init_db_schema(&pool).await?;
        Ok(Self { pool })
    }

    // 初始化数据库架构
    async fn init_db_schema(pool: &SqlitePool) -> Result<(), SqlxError> {
        // 创建连接表
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS connections (
                id TEXT,
                conn_download INTEGER,
                conn_upload INTEGER,
                last_updated TEXT,
                start TEXT,
                network TEXT,
                source_ip TEXT,
                destination_ip TEXT,
                source_port TEXT,
                destination_port TEXT,
                host TEXT,
                process TEXT,
                process_path TEXT,
                special_rules TEXT,
                chains TEXT,
                rule TEXT,
                rule_payload TEXT,
                agent_id TEXT,
                PRIMARY KEY (id, agent_id)
            )
            "#,
        )
        .await?;

        // 创建同步状态表（用于从机）
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS sync_state (
                id INTEGER PRIMARY KEY,
                last_sync_time TEXT,
                pending_records INTEGER,
                last_connection_id TEXT
            )
            "#,
        )
        .await?;

        // 初始化同步状态（如果尚未有记录）
        pool.execute(
            r#"
            INSERT OR IGNORE INTO sync_state (id, last_sync_time, pending_records, last_connection_id)
            VALUES (1, datetime('now', 'utc'), 0, '')
            "#,
        ).await?;

        // 创建连接关闭审计日志表
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS connection_logs (
                id TEXT,
                agent_id TEXT,
                source_ip TEXT,
                destination_ip TEXT,
                source_port TEXT,
                destination_port TEXT,
                host TEXT,
                rule TEXT,
                rule_payload TEXT,
                chains TEXT,
                network TEXT,
                process TEXT,
                process_path TEXT,
                download INTEGER,
                upload INTEGER,
                start TEXT,
                end TEXT,
                special_rules TEXT,
                synced INTEGER DEFAULT 0,
                PRIMARY KEY (id, agent_id)
            )
            "#,
        )
        .await?;

        // 为旧版数据库迁移：添加可能缺失的列
        let migration_columns = [("synced", "INTEGER DEFAULT 0")];
        for (col, ty) in &migration_columns {
            let alter_sql = format!(
                "ALTER TABLE connection_logs ADD COLUMN IF NOT EXISTS {} {}",
                col, ty
            );
            // 忽略重复列错误（旧表已存在该列时）
            if let Err(e) = pool.execute(alter_sql.as_str()).await {
                tracing::debug!(target: "backend::db::schema", "migration column {} skipped: {}", col, e);
            }
        }

        // 创建 connection_logs 索引
        pool.execute("CREATE INDEX IF NOT EXISTS idx_connection_logs_end ON connection_logs(end)")
            .await?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_logs_host ON connection_logs(host)",
        )
        .await?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_logs_rule ON connection_logs(rule)",
        )
        .await?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_logs_network ON connection_logs(network)",
        )
        .await?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_logs_synced ON connection_logs(synced, end)",
        )
        .await?;
        pool.execute(
            "CREATE INDEX IF NOT EXISTS idx_connection_logs_destination_port ON connection_logs(destination_port)",
        )
        .await?;

        Ok(())
    }

    // 通用的参数绑定方法
    fn bind_params<'a>(
        query: sqlx::query::Query<'a, Sqlite, sqlx::sqlite::SqliteArguments<'a>>,
        params: &'a [String],
    ) -> sqlx::query::Query<'a, Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
        let mut bound_query = query;
        for param in params {
            bound_query = bound_query.bind(param);
        }
        bound_query
    }

    // 通用查询执行方法
    async fn execute_query<F, T>(
        &self,
        sql: &str,
        params: &[String],
        mut row_mapper: F,
    ) -> QueryResult<Vec<T>>
    where
        F: FnMut(sqlx::sqlite::SqliteRow) -> QueryResult<T>,
    {
        let query = sqlx::query(sql);
        let query = Self::bind_params(query, params);
        let rows = query.fetch_all(&self.pool).await?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(row_mapper(row)?);
        }
        Ok(results)
    }

    // ==================== 同步相关方法 ====================

    // 获取当前所有活跃连接记录（用于全量同步）
    pub async fn get_active_records(&self, limit: i64) -> QueryResult<Vec<ConnectionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, conn_download, conn_upload, last_updated, start,
                network, source_ip, destination_ip,
                source_port, destination_port, host, process,
                process_path, special_rules, chains, rule, rule_payload, agent_id
            FROM connections
            ORDER BY last_updated ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(ConnectionRecord {
                id: row.try_get("id")?,
                download: row.try_get("conn_download")?,
                upload: row.try_get("conn_upload")?,
                last_updated: row.try_get("last_updated")?,
                start: row.try_get("start")?,
                network: row.try_get("network")?,
                source_ip: row.try_get("source_ip")?,
                destination_ip: row.try_get("destination_ip")?,
                source_port: row.try_get("source_port")?,
                destination_port: row.try_get("destination_port")?,
                host: row.try_get("host")?,
                process: row.try_get("process")?,
                process_path: row.try_get("process_path")?,
                special_rules: row.try_get("special_rules")?,
                chains: row.try_get("chains")?,
                rule: row.try_get("rule")?,
                rule_payload: row.try_get("rule_payload")?,
                agent_id: row.try_get("agent_id")?,
            });
        }
        Ok(records)
    }

    // 将连接记录保存到数据库中
    pub async fn upsert_connection_record<'a, E>(
        &self,
        exec: E,
        record: &ConnectionRecord,
    ) -> QueryResult<()>
    where
        E: Executor<'a, Database = Sqlite>,
    {
        sqlx_query(
            r#"
            INSERT INTO connections (
                id, conn_download, conn_upload, last_updated, start, network,
                source_ip, destination_ip, source_port, destination_port, host, process,
                process_path, special_rules, chains, rule, rule_payload, agent_id
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id, agent_id) DO UPDATE SET
                id=excluded.id,
                conn_download=excluded.conn_download,
                conn_upload=excluded.conn_upload,
                last_updated=excluded.last_updated,
                start=excluded.start,
                network=excluded.network,
                source_ip=excluded.source_ip,
                destination_ip=excluded.destination_ip,
                source_port=excluded.source_port,
                destination_port=excluded.destination_port,
                host=excluded.host,
                process=excluded.process,
                process_path=excluded.process_path,
                special_rules=excluded.special_rules,
                chains=excluded.chains,
                rule=excluded.rule,
                rule_payload=excluded.rule_payload
            "#,
        )
        .bind(&record.id)
        .bind(record.download)
        .bind(record.upload)
        .bind(&record.last_updated)
        .bind(&record.start)
        .bind(&record.network)
        .bind(&record.source_ip)
        .bind(&record.destination_ip)
        .bind(&record.source_port)
        .bind(&record.destination_port)
        .bind(&record.host)
        .bind(&record.process)
        .bind(&record.process_path)
        .bind(&record.special_rules)
        .bind(&record.chains)
        .bind(&record.rule)
        .bind(&record.rule_payload)
        .bind(&record.agent_id)
        .execute(exec)
        .await?;
        Ok(())
    }

    // 批量插入连接记录
    pub async fn batch_upsert_records(&self, records: &[ConnectionRecord]) -> QueryResult<()> {
        let mut tx = self.pool.begin().await?;
        for record in records {
            self.upsert_connection_record(&mut *tx, record).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ==================== 统计查询方法 ====================

    // 执行计数查询
    pub async fn execute_count_query(&self, sql: &str, params: &[String]) -> QueryResult<i64> {
        let query = sqlx::query(sql);
        let query = Self::bind_params(query, params);
        let row = query.fetch_one(&self.pool).await?;
        let count: i64 = row.try_get(0)?;
        Ok(count)
    }

    // 执行流量查询
    pub async fn execute_traffic_query(
        &self,
        sql: &str,
        params: &[String],
    ) -> QueryResult<(i64, i64)> {
        let query = sqlx::query(sql);
        let query = Self::bind_params(query, params);
        let row = query.fetch_one(&self.pool).await?;

        // 获取下载和上传流量，如果为空则返回0
        let download: Option<i64> = row.try_get(0)?;
        let upload: Option<i64> = row.try_get(1)?;

        Ok((download.unwrap_or(0), upload.unwrap_or(0)))
    }

    // 安全获取行的值（按索引），NULL 映射为 JsonValue::Null，不支持类型返回错误
    fn get_row_value_by_index(
        row: &sqlx::sqlite::SqliteRow,
        index: usize,
    ) -> QueryResult<JsonValue> {
        if index >= row.columns().len() {
            return Err(SqlxError::ColumnNotFound(format!(
                "index {} out of bounds (max {})",
                index,
                row.columns().len()
            )));
        }
        let col_name = row.column(index).name().to_string();
        Self::get_row_value_impl(row, index, col_name)
    }

    // 安全获取行的值（按列名），NULL 映射为 JsonValue::Null，不支持类型返回错误
    fn get_row_value_by_name(row: &sqlx::sqlite::SqliteRow, name: &str) -> QueryResult<JsonValue> {
        let index = row
            .columns()
            .iter()
            .position(|col| col.name() == name)
            .ok_or_else(|| SqlxError::ColumnNotFound(name.to_string()))?;
        Self::get_row_value_impl(row, index, name.to_string())
    }

    // get_row_value 公共实现逻辑
    #[inline]
    fn get_row_value_impl(
        row: &sqlx::sqlite::SqliteRow,
        index: usize,
        col_name: String,
    ) -> QueryResult<JsonValue> {
        match row.try_get::<Option<String>, _>(index) {
            Ok(val) => return Ok(val.map(JsonValue::String).unwrap_or(JsonValue::Null)),
            Err(SqlxError::ColumnNotFound(_)) => return Err(SqlxError::ColumnNotFound(col_name)),
            Err(SqlxError::ColumnDecode { .. }) => {}
            Err(e) => return Err(e),
        }
        match row.try_get::<Option<i64>, _>(index) {
            Ok(val) => {
                return Ok(val
                    .map(|v| JsonValue::Number(JsonNumber::from(v)))
                    .unwrap_or(JsonValue::Null))
            }
            Err(SqlxError::ColumnNotFound(_)) => return Err(SqlxError::ColumnNotFound(col_name)),
            Err(SqlxError::ColumnDecode { .. }) => {}
            Err(e) => return Err(e),
        }
        match row.try_get::<Option<f64>, _>(index) {
            Ok(val) => return Ok(val.map(JsonValue::from).unwrap_or(JsonValue::Null)),
            Err(SqlxError::ColumnNotFound(_)) => return Err(SqlxError::ColumnNotFound(col_name)),
            Err(SqlxError::ColumnDecode { .. }) => {}
            Err(e) => return Err(e),
        }
        Err(SqlxError::ColumnDecode {
            index: col_name.clone(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unsupported SQLite type for column {}", col_name),
            )),
        })
    }

    // 执行分组查询
    pub async fn execute_group_query(
        &self,
        sql: &str,
        params: &[String],
        key_name: Option<&str>,
    ) -> JsonResult {
        // 执行查询
        let rows = self.execute_query(sql, params, Ok).await?;

        // 转换结果为JSON数组
        let mut results = Vec::with_capacity(rows.len());

        for row in rows {
            // 灵活处理不同类型的分组字段
            let mut result_obj = serde_json::Map::new();

            // 标准的单列分组情况
            self.process_standard_group(&row, &mut result_obj, key_name)?;

            // 添加统计计数
            self.add_count_metrics(&row, &mut result_obj)?;

            results.push(JsonValue::Object(result_obj));
        }

        Ok(results)
    }

    // 处理标准分组结果
    fn process_standard_group(
        &self,
        row: &sqlx::sqlite::SqliteRow,
        result_obj: &mut serde_json::Map<String, JsonValue>,
        key_name: Option<&str>,
    ) -> QueryResult<()> {
        // 获取分组键
        let group_key = Self::get_row_value_by_index(row, 0)?;

        if let Some(key_name) = key_name {
            result_obj.insert(key_name.to_string(), group_key);
        }
        Ok(())
    }

    // 添加统计指标（fail fast：列缺失或类型不匹配时返回错误）
    fn add_count_metrics(
        &self,
        row: &sqlx::sqlite::SqliteRow,
        result_obj: &mut serde_json::Map<String, JsonValue>,
    ) -> QueryResult<()> {
        // 如果已经添加了count字段(在规则组合处理中)，则跳过
        if result_obj.contains_key("count") {
            return Ok(());
        }

        let count: i64 = row.try_get::<Option<i64>, _>(1)?.unwrap_or(0);
        let download: i64 = row.try_get::<Option<i64>, _>(2)?.unwrap_or(0);
        let upload: i64 = row.try_get::<Option<i64>, _>(3)?.unwrap_or(0);

        result_obj.insert("count".to_string(), JsonValue::Number(count.into()));
        result_obj.insert("download".to_string(), JsonValue::Number(download.into()));
        result_obj.insert("upload".to_string(), JsonValue::Number(upload.into()));
        result_obj.insert(
            "total".to_string(),
            JsonValue::Number((download + upload).into()),
        );
        Ok(())
    }

    // 处理chains分组结果
    pub async fn process_chains_results(
        &self,
        results: Vec<JsonValue>,
        sort_by: Option<&str>,
        sort_order: Option<&str>,
        limit: Option<u32>,
    ) -> JsonResult {
        let mut processed_results = Vec::new();

        for result in results {
            if let Some(obj) = result.as_object() {
                if let Some(chains_value) = obj.get("chains") {
                    if let Some(chains_str) = chains_value.as_str() {
                        // 解析chains字符串
                        let mut node = chains_str.to_string();

                        // 尝试解析JSON数组
                        if let Ok(array) = serde_json::from_str::<Vec<String>>(chains_str.trim()) {
                            if let Some(last) = array.last() {
                                node = last.clone();
                            }
                        } else {
                            tracing::warn!(target: "backend::db", "无法解析 chains JSON: {}", chains_str);
                        }

                        // 创建节点结果
                        let count = obj.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                        let download = obj.get("download").and_then(|v| v.as_i64()).unwrap_or(0);
                        let upload = obj.get("upload").and_then(|v| v.as_i64()).unwrap_or(0);

                        // 将结果聚合到节点统计中
                        self.aggregate_node_stats(
                            &mut processed_results,
                            &node,
                            count,
                            download,
                            upload,
                        )?;
                    }
                }
            }
        }

        // 排序处理
        self.sort_and_limit_results(&mut processed_results, sort_by, sort_order, limit);

        Ok(processed_results)
    }

    // 聚合节点统计数据
    fn aggregate_node_stats(
        &self,
        results: &mut Vec<JsonValue>,
        node: &str,
        count: i64,
        download: i64,
        upload: i64,
    ) -> Result<(), SqlxError> {
        // 查找是否已有该节点的统计
        let mut found = false;
        for result in results.iter_mut() {
            if let Some(obj) = result.as_object_mut() {
                if let Some(node_val) = obj.get("node") {
                    if node_val.as_str() == Some(node) {
                        found = true;

                        let entry_count = obj.get("count").and_then(|v| v.as_i64());
                        let entry_download = obj.get("download").and_then(|v| v.as_i64());
                        let entry_upload = obj.get("upload").and_then(|v| v.as_i64());

                        if let (Some(c), Some(d), Some(u)) =
                            (entry_count, entry_download, entry_upload)
                        {
                            obj.insert("count".to_string(), JsonValue::Number((c + count).into()));
                            obj.insert(
                                "download".to_string(),
                                JsonValue::Number((d + download).into()),
                            );
                            obj.insert(
                                "upload".to_string(),
                                JsonValue::Number((u + upload).into()),
                            );
                            obj.insert(
                                "total".to_string(),
                                JsonValue::Number((d + download + u + upload).into()),
                            );
                        } else {
                            let err = format!(
                                "corrupted aggregate entry for node {}: count={:?} download={:?} upload={:?}",
                                node, entry_count, entry_download, entry_upload
                            );
                            tracing::error!(target: "backend::db", "{}", err);
                            return Err(SqlxError::Protocol(err));
                        }

                        break;
                    }
                }
            }
        }

        // 如果没有找到，添加新的节点统计
        if !found {
            results.push(serde_json::json!({
                "node": node,
                "count": count,
                "download": download,
                "upload": upload,
                "total": download + upload
            }));
        }
        Ok(())
    }

    // 排序和限制结果
    fn sort_and_limit_results(
        &self,
        results: &mut Vec<JsonValue>,
        sort_by: Option<&str>,
        sort_order: Option<&str>,
        limit: Option<u32>,
    ) {
        // 应用排序
        if let Some(sort_field) = sort_by {
            let ascending = sort_order == Some("asc");

            results.sort_by(|a, b| {
                let a_val = a.get(sort_field).and_then(|v| v.as_i64());
                let b_val = b.get(sort_field).and_then(|v| v.as_i64());
                match (a_val, b_val) {
                    (Some(a), Some(b)) => {
                        if ascending {
                            a.cmp(&b)
                        } else {
                            b.cmp(&a)
                        }
                    }
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                }
            });
        } else {
            // 默认按count降序排序
            results.sort_by(|a, b| {
                let a_count = a.get("count").and_then(|v| v.as_i64());
                let b_count = b.get("count").and_then(|v| v.as_i64());
                match (a_count, b_count) {
                    (Some(a), Some(b)) => b.cmp(&a),
                    (None, None) => std::cmp::Ordering::Equal,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                }
            });
        }

        // 应用限制
        if let Some(limit) = limit {
            let limit = limit as usize;
            if limit < results.len() {
                results.truncate(limit);
            }
        }
    }

    // 处理GEOIP分组统计

    pub async fn execute_host_group_query(&self, sql: &str, params: &[String]) -> JsonResult {
        self.execute_query(sql, params, |row| {
            let mut result_obj = serde_json::Map::new();

            let host: String = row.try_get("host_display")?;
            let count: i64 = row.try_get("count")?;
            let download: i64 = row.try_get("download")?;
            let upload: i64 = row.try_get("upload")?;

            result_obj.insert("host".to_string(), JsonValue::String(host));
            result_obj.insert("count".to_string(), JsonValue::Number(count.into()));
            result_obj.insert("download".to_string(), JsonValue::Number(download.into()));
            result_obj.insert("upload".to_string(), JsonValue::Number(upload.into()));
            result_obj.insert(
                "total".to_string(),
                JsonValue::Number((download + upload).into()),
            );

            Ok(JsonValue::Object(result_obj))
        })
        .await
    }

    // 专门处理 destination 分组查询
    pub async fn execute_destination_group_query(
        &self,
        sql: &str,
        params: &[String],
    ) -> JsonResult {
        self.execute_query(sql, params, |row| {
            let mut result_obj = serde_json::Map::new();

            let host: String = row.try_get("host_display")?;
            let destination_ip: String = row.try_get("destination_ip")?;
            let count: i64 = row.try_get("count")?;
            let download: i64 = row.try_get("download")?;
            let upload: i64 = row.try_get("upload")?;

            result_obj.insert("host_display".to_string(), JsonValue::String(host));
            result_obj.insert(
                "destination_ip".to_string(),
                JsonValue::String(destination_ip),
            );
            result_obj.insert("count".to_string(), JsonValue::Number(count.into()));
            result_obj.insert("download".to_string(), JsonValue::Number(download.into()));
            result_obj.insert("upload".to_string(), JsonValue::Number(upload.into()));
            result_obj.insert(
                "total".to_string(),
                JsonValue::Number((download + upload).into()),
            );

            Ok(JsonValue::Object(result_obj))
        })
        .await
    }

    // 专门处理 source 分组查询
    pub async fn execute_source_group_query(&self, sql: &str, params: &[String]) -> JsonResult {
        self.execute_query(sql, params, |row| {
            let mut result_obj = serde_json::Map::new();

            let host: String = row.try_get("host_display")?;
            let source_ip: String = row.try_get("source_ip")?;
            let count: i64 = row.try_get("count")?;
            let download: i64 = row.try_get("download")?;
            let upload: i64 = row.try_get("upload")?;

            result_obj.insert("host_display".to_string(), JsonValue::String(host));
            result_obj.insert("source_ip".to_string(), JsonValue::String(source_ip));
            result_obj.insert("count".to_string(), JsonValue::Number(count.into()));
            result_obj.insert("download".to_string(), JsonValue::Number(download.into()));
            result_obj.insert("upload".to_string(), JsonValue::Number(upload.into()));
            result_obj.insert(
                "total".to_string(),
                JsonValue::Number((download + upload).into()),
            );

            Ok(JsonValue::Object(result_obj))
        })
        .await
    }

    // 执行时间序列查询
    pub async fn execute_timeseries_query(&self, sql: &str, params: &[String]) -> JsonResult {
        let query = sqlx::query(sql);
        let query = Self::bind_params(query, params);
        let rows = query.fetch_all(&self.pool).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let time_point: String = row.try_get(0)?;
            let value: i64 = row.try_get(1)?;
            results.push(serde_json::json!({
                "time": time_point,
                "value": value
            }));
        }
        Ok(results)
    }

    // 执行连接查询
    pub async fn execute_connections_query(&self, sql: &str, params: &[String]) -> JsonResult {
        self.execute_query(sql, params, |row| {
            let mut result_obj = serde_json::Map::new();

            // 添加所有标准字段，NULL 显式映射为 JsonValue::Null，类型不匹配时返回错误
            for field in &[
                "id",
                "download",
                "upload",
                "last_updated",
                "start",
                "network",
                "source_ip",
                "destination_ip",
                "source_port",
                "destination_port",
                "host",
                "process",
                "process_path",
                "special_rules",
                "chains",
                "rule",
                "rule_payload",
                "agent_id",
            ] {
                let value = Self::get_row_value_by_name(&row, field)?;
                result_obj.insert(field.to_string(), value);
            }

            Ok(JsonValue::Object(result_obj))
        })
        .await
    }

    // 执行筛选选项查询
    pub async fn execute_filter_options_query(&self, sql: &str, params: &[String]) -> JsonResult {
        self.execute_query(sql, params, |row| {
            // 获取第一列的值作为选项
            let column_name = row.column(0).name();
            let value = Self::get_row_value_by_index(&row, 0)?;

            // 为空值使用特殊标记，以便前端可以显示
            let display_value = match &value {
                JsonValue::String(s) if s.is_empty() => JsonValue::String("(空)".to_string()),
                _ => value,
            };

            let mut result_obj = serde_json::Map::new();
            result_obj.insert("value".to_string(), display_value.clone());
            result_obj.insert("label".to_string(), display_value);
            result_obj.insert(
                "field".to_string(),
                JsonValue::String(column_name.to_string()),
            );

            Ok(JsonValue::Object(result_obj))
        })
        .await
    }

    // ==================== 代理节点相关方法 ====================

    // 获取所有代理节点列表
    pub async fn get_agents(&self, exclude_rule: Option<&str>) -> JsonResult {
        let mut sql = r#"
            SELECT
                agent_id,
                MAX(end) as last_active,
                COUNT(*) as connections_count,
                SUM(download) as total_download,
                SUM(upload) as total_upload
            FROM connection_logs
            WHERE agent_id IS NOT NULL
        "#
        .to_string();
        let mut params: Vec<String> = Vec::new();

        if let Some(rule) = exclude_rule {
            sql.push_str(" AND COALESCE(rule, '') != ?");
            params.push(rule.to_string());
        }

        sql.push_str(" GROUP BY agent_id ORDER BY last_active DESC");

        self.execute_query(&sql, &params, |row| {
            let agent_id: String = row.try_get("agent_id")?;
            let last_active: String = row.try_get("last_active")?;
            let connections_count: i64 = row.try_get("connections_count")?;
            let total_download: i64 = row
                .try_get::<Option<i64>, _>("total_download")?
                .unwrap_or(0);
            let total_upload: i64 = row.try_get::<Option<i64>, _>("total_upload")?.unwrap_or(0);

            Ok(serde_json::json!({
                "id": agent_id,
                "last_active": last_active,
                "connections_count": connections_count,
                "total_download": total_download,
                "total_upload": total_upload,
                "total_traffic": total_download + total_upload,
                "status": "unknown"
            }))
        })
        .await
    }

    // 获取单个代理节点状态
    pub async fn get_agent_status(
        &self,
        agent_id: &str,
        exclude_rule: Option<&str>,
    ) -> QueryResult<JsonValue> {
        let mut sql = r#"
            SELECT
                agent_id,
                MAX(end) as last_active,
                COUNT(*) as connections_count,
                SUM(download) as total_download,
                SUM(upload) as total_upload
            FROM connection_logs
            WHERE agent_id = ?
        "#
        .to_string();
        let mut params: Vec<String> = vec![agent_id.to_string()];

        if let Some(rule) = exclude_rule {
            sql.push_str(" AND COALESCE(rule, '') != ?");
            params.push(rule.to_string());
        }

        sql.push_str(" GROUP BY agent_id");

        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        let row_result = query.fetch_optional(&self.pool).await?;

        if let Some(row) = row_result {
            let last_active: String = row.try_get("last_active")?;
            let connections_count: i64 = row.try_get("connections_count")?;
            let total_download: i64 = row
                .try_get::<Option<i64>, _>("total_download")?
                .unwrap_or(0);
            let total_upload: i64 = row.try_get::<Option<i64>, _>("total_upload")?.unwrap_or(0);

            let now = Utc::now();
            let is_active = if last_active.is_empty() {
                false
            } else {
                match chrono::DateTime::parse_from_rfc3339(&last_active) {
                    Ok(dt) => {
                        let last_active_time = dt.with_timezone(&Utc);
                        now.signed_duration_since(last_active_time).num_minutes() < 10
                    }
                    Err(e) => {
                        tracing::error!(target: "backend::db", "无法解析 agent {} 的 last_active '{}': {}", agent_id, last_active, e);
                        return Err(SqlxError::ColumnDecode {
                            index: "last_active".to_string(),
                            source: Box::new(e),
                        });
                    }
                }
            };

            let networks = self.get_agent_networks(agent_id, exclude_rule).await?;
            let rules = self.get_agent_rules(agent_id, exclude_rule).await?;

            Ok(serde_json::json!({
                "id": agent_id,
                "last_active": last_active,
                "connections_count": connections_count,
                "total_download": total_download,
                "total_upload": total_upload,
                "total_traffic": total_download + total_upload,
                "is_active": is_active,
                "status": if is_active { "active" } else { "inactive" },
                "networks": networks,
                "rules": rules
            }))
        } else {
            Err(SqlxError::RowNotFound)
        }
    }

    // 获取代理节点的网络类型分布
    async fn get_agent_networks(&self, agent_id: &str, exclude_rule: Option<&str>) -> JsonResult {
        let mut sql = r#"
            SELECT
                network,
                COUNT(*) as count
            FROM connection_logs
            WHERE agent_id = ?
        "#
        .to_string();
        let mut params: Vec<String> = vec![agent_id.to_string()];

        if let Some(rule) = exclude_rule {
            sql.push_str(" AND COALESCE(rule, '') != ?");
            params.push(rule.to_string());
        }

        sql.push_str(" GROUP BY network");

        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let network: String = row.try_get("network")?;
            let count: i64 = row.try_get("count")?;
            results.push(serde_json::json!({
                "network": network,
                "count": count
            }));
        }
        Ok(results)
    }

    // 获取代理节点的规则分布
    async fn get_agent_rules(&self, agent_id: &str, exclude_rule: Option<&str>) -> JsonResult {
        let mut sql = r#"
            SELECT
                rule,
                COUNT(*) as count
            FROM connection_logs
            WHERE agent_id = ?
        "#
        .to_string();
        let mut params: Vec<String> = vec![agent_id.to_string()];

        if let Some(rule) = exclude_rule {
            sql.push_str(" AND COALESCE(rule, '') != ?");
            params.push(rule.to_string());
        }

        sql.push_str(" GROUP BY rule");

        let mut query = sqlx::query(&sql);
        for p in &params {
            query = query.bind(p);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let rule: String = row.try_get("rule")?;
            let count: i64 = row.try_get("count")?;
            results.push(serde_json::json!({
                "rule": rule,
                "count": count
            }));
        }
        Ok(results)
    }

    // ==================== 流量审计日志方法 ====================

    // 插入连接关闭审计日志
    async fn insert_connection_log_with_executor<'a, E>(
        &self,
        exec: E,
        record: &ConnectionLog,
    ) -> QueryResult<()>
    where
        E: Executor<'a, Database = Sqlite>,
    {
        sqlx_query(
            r#"
            INSERT INTO connection_logs (
                id, agent_id, source_ip, destination_ip, source_port, destination_port,
                host, rule, rule_payload, chains, network, process, process_path,
                download, upload, start, end, special_rules, synced
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id, agent_id) DO UPDATE SET
                id=excluded.id,
                source_ip=excluded.source_ip,
                destination_ip=excluded.destination_ip,
                source_port=excluded.source_port,
                destination_port=excluded.destination_port,
                host=excluded.host,
                rule=excluded.rule,
                rule_payload=excluded.rule_payload,
                chains=excluded.chains,
                network=excluded.network,
                process=excluded.process,
                process_path=excluded.process_path,
                download=excluded.download,
                upload=excluded.upload,
                start=excluded.start,
                end=excluded.end,
                special_rules=excluded.special_rules,
                synced=excluded.synced
            "#,
        )
        .bind(&record.id)
        .bind(&record.agent_id)
        .bind(&record.source_ip)
        .bind(&record.destination_ip)
        .bind(&record.source_port)
        .bind(&record.destination_port)
        .bind(&record.host)
        .bind(&record.rule)
        .bind(&record.rule_payload)
        .bind(&record.chains)
        .bind(&record.network)
        .bind(&record.process)
        .bind(&record.process_path)
        .bind(record.download)
        .bind(record.upload)
        .bind(&record.start)
        .bind(&record.end)
        .bind(&record.special_rules)
        .bind(record.synced.unwrap_or(0))
        .execute(exec)
        .await?;
        Ok(())
    }

    pub async fn insert_connection_log(&self, record: &ConnectionLog) -> QueryResult<()> {
        self.insert_connection_log_with_executor(&self.pool, record)
            .await
    }

    // 批量插入/更新连接审计日志
    pub async fn batch_insert_connection_logs(&self, records: &[ConnectionLog]) -> QueryResult<()> {
        let mut tx = self.pool.begin().await?;
        for record in records {
            self.insert_connection_log_with_executor(&mut *tx, record)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // 查询历史流量审计日志（支持筛选、排序、分页）
    #[allow(clippy::too_many_arguments)]
    pub async fn query_connection_logs(
        &self,
        agent_id: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        source_ip: Option<&str>,
        host: Option<&str>,
        rule: Option<&str>,
        network: Option<&str>,
        keyword: Option<&str>,
        sort_by: Option<&str>,
        sort_order: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> QueryResult<(Vec<ConnectionLog>, i64)> {
        let mut where_clauses = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(agent_id) = agent_id {
            where_clauses.push("agent_id = ?".to_string());
            params.push(agent_id.to_string());
        }
        if let Some(from) = from {
            where_clauses.push("end >= ?".to_string());
            params.push(from.to_string());
        }
        if let Some(to) = to {
            where_clauses.push("end <= ?".to_string());
            params.push(to.to_string());
        }
        if let Some(source_ip) = source_ip {
            where_clauses.push("source_ip = ?".to_string());
            params.push(source_ip.to_string());
        }
        if let Some(host) = host {
            where_clauses.push("host LIKE ? ESCAPE '\\'".to_string());
            params.push(format!(
                "%{}%",
                host.replace('%', "\\%").replace('_', "\\_")
            ));
        }
        if let Some(rule) = rule {
            where_clauses.push("rule = ?".to_string());
            params.push(rule.to_string());
        }
        if let Some(network) = network {
            where_clauses.push("network = ?".to_string());
            params.push(network.to_string());
        }
        if let Some(keyword) = keyword {
            where_clauses.push(
                "(host LIKE ? ESCAPE '\\' OR rule LIKE ? ESCAPE '\\' OR process LIKE ? ESCAPE '\\' OR destination_ip LIKE ? ESCAPE '\\' OR source_ip LIKE ? ESCAPE '\\' OR chains LIKE ? ESCAPE '\\')".to_string()
            );
            let pattern = format!("%{}%", keyword.replace('%', "\\%").replace('_', "\\_"));
            for _ in 0..6 {
                params.push(pattern.clone());
            }
        }

        let where_sql = if where_clauses.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sort_field = match sort_by {
            Some("download") => "download",
            Some("upload") => "upload",
            Some("total") => "(download + upload)",
            _ => "end",
        };
        let sort_dir = if sort_order == Some("asc") {
            "ASC"
        } else {
            "DESC"
        };

        // 查询总数
        let count_sql = format!("SELECT COUNT(*) FROM connection_logs {}", where_sql);
        let count_query = sqlx::query(&count_sql);
        let count_query = Self::bind_params(count_query, &params);
        let count_row = count_query.fetch_one(&self.pool).await?;
        let total: i64 = count_row.try_get(0)?;

        // 查询数据
        let data_sql = format!(
            "SELECT id, agent_id, source_ip, destination_ip, source_port, destination_port, host, rule, rule_payload, chains, network, process, process_path, download, upload, start, end, special_rules, synced
             FROM connection_logs {}
             ORDER BY {} {}
             LIMIT ? OFFSET ?",
            where_sql, sort_field, sort_dir
        );
        let mut data_params = params.clone();
        data_params.push(limit.to_string());
        data_params.push(offset.to_string());

        let data_query = sqlx::query(&data_sql);
        let data_query = Self::bind_params(data_query, &data_params);
        let rows = data_query.fetch_all(&self.pool).await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(ConnectionLog {
                id: row.try_get("id")?,
                agent_id: row.try_get("agent_id")?,
                source_ip: row.try_get("source_ip")?,
                destination_ip: row.try_get("destination_ip")?,
                source_port: row.try_get("source_port")?,
                destination_port: row.try_get("destination_port")?,
                host: row.try_get("host")?,
                rule: row.try_get("rule")?,
                rule_payload: row.try_get("rule_payload")?,
                chains: row.try_get("chains")?,
                network: row.try_get("network")?,
                process: row.try_get("process")?,
                process_path: row.try_get("process_path")?,
                download: row.try_get("download")?,
                upload: row.try_get("upload")?,
                start: row.try_get("start")?,
                end: row.try_get("end")?,
                special_rules: row.try_get("special_rules")?,
                synced: row.try_get::<Option<i32>, _>("synced")?.or(Some(0)),
            });
        }
        Ok((items, total))
    }

    // 标记 connection_logs 为已同步
    pub async fn mark_connection_logs_synced(&self, ids: &[(String, String)]) -> QueryResult<()> {
        let mut tx = self.pool.begin().await?;
        for (id, agent_id) in ids {
            sqlx::query("UPDATE connection_logs SET synced = 1 WHERE id = ? AND agent_id = ?")
                .bind(id)
                .bind(agent_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // 批量删除 connections（用于关闭即删）
    // 使用多值 IN 列表批量删除，每批最多 999 对（受 SQLite 表达式数量上限限制）
    // 任何 chunk 失败都会导致整个事务回滚
    pub async fn batch_delete_records(&self, ids: &[(String, String)]) -> QueryResult<i64> {
        let mut tx = self.pool.begin().await?;

        for chunk in ids.chunks(999) {
            let placeholders = chunk.iter().map(|_| "(?,?)").collect::<Vec<_>>().join(",");
            let sql = format!(
                "DELETE FROM connections WHERE (id, agent_id) IN ({})",
                placeholders
            );
            let mut query = sqlx::query(&sql);
            for (id, agent_id) in chunk {
                query = query.bind(id).bind(agent_id);
            }
            query.execute(&mut *tx).await?;
        }

        tx.commit().await?;
        Ok(ids.len() as i64)
    }

    // 全量替换某个 agent 的 connections（事务内先删后插）
    pub async fn replace_connections_for_agent(
        &self,
        agent_id: &str,
        records: &[ConnectionRecord],
    ) -> QueryResult<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM connections WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&mut *tx)
            .await?;
        for record in records {
            self.upsert_connection_record(&mut *tx, record).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // 清理已同步的 connection_logs（立即删除）
    pub async fn cleanup_synced_connection_logs(&self) -> QueryResult<i64> {
        let result = sqlx::query("DELETE FROM connection_logs WHERE synced = 1")
            .execute(&self.pool)
            .await?;
        let deleted = result.rows_affected() as i64;
        if deleted > 0 {
            tracing::info!(target: "backend::db::cleanup", "cleaned up {} synced connection_logs", deleted);
        }
        Ok(deleted)
    }

    pub async fn vacuum_db(&self) -> QueryResult<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_pending_connection_logs(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> QueryResult<Vec<ConnectionLog>> {
        let sql = r#"
            SELECT id, agent_id, source_ip, destination_ip, source_port, destination_port,
                   host, rule, rule_payload, chains, network, process, process_path,
                   download, upload, start, end, special_rules, synced
            FROM connection_logs
            WHERE agent_id = ? AND synced = 0
            ORDER BY end ASC
            LIMIT ?
        "#;
        let rows = sqlx::query(sql)
            .bind(agent_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        let items = rows
            .into_iter()
            .map(|row| {
                Ok(ConnectionLog {
                    id: row.try_get("id")?,
                    agent_id: row.try_get("agent_id")?,
                    source_ip: row.try_get("source_ip").unwrap_or_default(),
                    destination_ip: row.try_get("destination_ip").unwrap_or_default(),
                    source_port: row.try_get("source_port").unwrap_or_default(),
                    destination_port: row.try_get("destination_port").unwrap_or_default(),
                    host: row.try_get("host").unwrap_or_default(),
                    rule: row.try_get("rule").unwrap_or_default(),
                    rule_payload: row.try_get("rule_payload").unwrap_or_default(),
                    chains: row.try_get("chains").unwrap_or_default(),
                    network: row.try_get("network").unwrap_or_default(),
                    process: row.try_get("process").unwrap_or_default(),
                    process_path: row.try_get("process_path").unwrap_or_default(),
                    download: row.try_get("download").unwrap_or(0),
                    upload: row.try_get("upload").unwrap_or(0),
                    start: row.try_get("start").unwrap_or_default(),
                    end: row.try_get("end").unwrap_or_default(),
                    special_rules: row.try_get("special_rules").unwrap_or_default(),
                    synced: row.try_get::<Option<i32>, _>("synced").unwrap_or(Some(0)),
                })
            })
            .collect::<QueryResult<Vec<_>>>()?;
        Ok(items)
    }

    pub async fn cleanup_old_unsynced_connection_logs(&self, days: i64) -> QueryResult<i64> {
        if days <= 0 {
            return Err(SqlxError::Protocol("days must be positive".to_string()));
        }
        let sql = r#"
            DELETE FROM connection_logs
            WHERE synced = 0 AND datetime(end) < datetime('now', ?)
        "#;
        let days_param = format!("-{} day", days);
        let result = sqlx::query(sql)
            .bind(days_param)
            .execute(&self.pool)
            .await?;
        let deleted = result.rows_affected() as i64;
        if deleted > 0 {
            tracing::info!(target: "backend::db::cleanup", "cleaned up {} unsynced old connection_logs", deleted);
        }
        Ok(deleted)
    }

    // 清理所有超期的 connection_logs（无论 synced 状态）
    pub async fn cleanup_old_connection_logs(&self, days: i64) -> QueryResult<i64> {
        if days <= 0 {
            return Err(SqlxError::Protocol("days must be positive".to_string()));
        }
        let sql = r#"
            DELETE FROM connection_logs
            WHERE datetime(end) < datetime('now', ?)
        "#;
        let days_param = format!("-{} day", days);
        let result = sqlx::query(sql)
            .bind(days_param)
            .execute(&self.pool)
            .await?;
        let deleted = result.rows_affected() as i64;
        if deleted > 0 {
            tracing::info!(target: "backend::db::cleanup", "cleaned up {} old connection_logs", deleted);
        }
        Ok(deleted)
    }

    // ==================== 维护方法 ====================

    // 清理已同步且超过保留期限的旧数据
    pub async fn cleanup_old_records(&self, days_to_keep: i64) -> QueryResult<i64> {
        if days_to_keep <= 0 {
            return Err(SqlxError::Protocol(
                "days_to_keep must be positive".to_string(),
            ));
        }
        // 删除条件：超过保留期限（start < 当前时间 - days_to_keep天），无论是否已同步
        let sql = r#"
            DELETE FROM connections
            WHERE datetime(start) < datetime('now', ?, 'utc')
        "#;

        let days_param = format!("-{} day", days_to_keep);

        // 执行删除操作
        let result = sqlx::query(sql)
            .bind(days_param)
            .execute(&self.pool)
            .await?;

        let deleted = result.rows_affected() as i64;
        tracing::info!(target: "backend::db::cleanup", "cleaned up {} old connections", deleted);
        Ok(deleted)
    }

    // 按 last_updated 清理长时间未更新的 connections（用于 Master 实时连接兜底）
    pub async fn cleanup_stale_connections(&self, days_to_keep: i64) -> QueryResult<i64> {
        if days_to_keep <= 0 {
            return Err(SqlxError::Protocol(
                "days_to_keep must be positive".to_string(),
            ));
        }
        let sql = r#"
            DELETE FROM connections
            WHERE datetime(last_updated) < datetime('now', ?, 'utc')
        "#;

        let days_param = format!("-{} day", days_to_keep);

        let result = sqlx::query(sql)
            .bind(days_param)
            .execute(&self.pool)
            .await?;

        let deleted = result.rows_affected() as i64;
        if deleted > 0 {
            tracing::info!(target: "backend::db::cleanup", "cleaned up {} stale connections", deleted);
        }
        Ok(deleted)
    }
}

// 添加过滤条件 (完整版本，支持通配符和非通配符)
pub fn add_filter_condition_with_wildcards(
    sql: &mut String,
    params: &mut Vec<String>,
    field: &str,
    value: &Option<String>,
    operator: &str,
    use_wildcards: bool,
) {
    if let Some(value) = value {
        if use_wildcards && operator == "LIKE" {
            sql.push_str(&format!(" AND {} {} ? ESCAPE '\\\\'", field, operator));
            params.push(format!(
                "%{}%",
                value.replace('%', "\\%").replace('_', "\\_")
            ));
        } else {
            sql.push_str(&format!(" AND {} {} ?", field, operator));
            params.push(value.clone());
        }
    }
}

// 添加过滤条件 (简化版本，无通配符)
pub fn add_filter_condition(
    sql: &mut String,
    params: &mut Vec<String>,
    field: &str,
    value: &Option<String>,
    operator: &str,
) {
    if let Some(value) = value {
        sql.push_str(&format!(" AND {} {} ?", field, operator));
        params.push(value.clone());
    }
}
