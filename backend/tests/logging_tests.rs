#[allow(dead_code)]
#[path = "../src/common.rs"]
mod common;
#[allow(dead_code)]
#[path = "../src/db.rs"]
mod db;

use common::{ConnectionLog, ConnectionRecord};
use db::Database;
use sqlx::Row;
use uuid::Uuid;

struct TempDb {
    path: String,
    pub db: Database,
}

impl TempDb {
    async fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("mihomo-logging-test-{}.db", Uuid::new_v4()));
        let path_str = path.to_string_lossy().to_string();
        let db = Database::new(&path_str).await.expect("create db");
        Self { path: path_str, db }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(format!("{}-shm", self.path));
        let _ = std::fs::remove_file(format!("{}-wal", self.path));
    }
}

fn make_log_with_agent(
    id: &str,
    agent_id: &str,
    host: &str,
    end: &str,
    synced: i32,
) -> ConnectionLog {
    ConnectionLog {
        id: id.to_string(),
        agent_id: agent_id.to_string(),
        source_ip: "192.168.1.2".to_string(),
        destination_ip: "8.8.8.8".to_string(),
        source_port: "12345".to_string(),
        destination_port: "443".to_string(),
        host: host.to_string(),
        rule: "PROXY".to_string(),
        rule_payload: "".to_string(),
        chains: "node-a->node-b".to_string(),
        network: "tcp".to_string(),
        process: "chrome.exe".to_string(),
        process_path: "".to_string(),
        download: 2048,
        upload: 1024,
        start: "2026-04-16T08:00:00Z".to_string(),
        end: end.to_string(),
        special_rules: "".to_string(),
        synced: Some(synced),
    }
}

fn make_log(id: &str, host: &str, end: &str, synced: i32) -> ConnectionLog {
    make_log_with_agent(id, "agent-test", host, end, synced)
}

fn make_connection_record(
    id: &str,
    agent_id: &str,
    start: &str,
    last_updated: &str,
) -> ConnectionRecord {
    ConnectionRecord {
        id: id.to_string(),
        download: 0,
        upload: 0,
        last_updated: last_updated.to_string(),
        start: start.to_string(),
        network: "tcp".to_string(),
        source_ip: "192.168.1.2".to_string(),
        destination_ip: "8.8.8.8".to_string(),
        source_port: "12345".to_string(),
        destination_port: "443".to_string(),
        host: "example.com".to_string(),
        process: "".to_string(),
        process_path: "".to_string(),
        special_rules: "".to_string(),
        chains: "".to_string(),
        rule: "DIRECT".to_string(),
        rule_payload: "".to_string(),
        agent_id: Some(agent_id.to_string()),
    }
}

#[tokio::test]
async fn connection_logs_insert_and_query_should_work() {
    let temp = TempDb::new().await;
    temp.db
        .insert_connection_log(&make_log("c1", "dns.google", "2026-04-16T08:05:00Z", 0))
        .await
        .expect("insert c1");
    temp.db
        .insert_connection_log(&make_log("c2", "example.com", "2026-04-16T08:06:00Z", 0))
        .await
        .expect("insert c2");

    let (items, total) = temp
        .db
        .query_connection_logs(
            Some("agent-test"),
            None,
            None,
            None,
            Some("google"),
            None,
            Some("tcp"),
            None,
            Some("end"),
            Some("desc"),
            20,
            0,
        )
        .await
        .expect("query logs");

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].host, "dns.google");
}

#[tokio::test]
async fn connection_logs_batch_sync_round_trip_should_work() {
    let temp = TempDb::new().await;
    temp.db
        .insert_connection_log(&make_log("c1", "a.test", "2026-04-16T09:00:00Z", 0))
        .await
        .expect("insert c1");
    temp.db
        .insert_connection_log(&make_log("c2", "b.test", "2026-04-16T09:01:00Z", 0))
        .await
        .expect("insert c2");

    let pending = temp
        .db
        .get_pending_connection_logs("agent-test", 1000)
        .await
        .expect("pending logs");
    assert_eq!(pending.len(), 2);

    let ids: Vec<(String, String)> = pending
        .iter()
        .map(|log| (log.id.clone(), log.agent_id.clone()))
        .collect();
    temp.db
        .mark_connection_logs_synced(&ids)
        .await
        .expect("mark synced");

    let pending_after = temp
        .db
        .get_pending_connection_logs("agent-test", 1000)
        .await
        .expect("pending logs after");
    assert!(pending_after.is_empty());
}

#[tokio::test]
async fn connection_logs_pagination_should_work() {
    let temp = TempDb::new().await;
    temp.db
        .insert_connection_log(&make_log("c1", "a.test", "2026-04-16T10:00:00Z", 1))
        .await
        .expect("insert c1");
    temp.db
        .insert_connection_log(&make_log("c2", "b.test", "2026-04-16T10:01:00Z", 1))
        .await
        .expect("insert c2");
    temp.db
        .insert_connection_log(&make_log("c3", "c.test", "2026-04-16T10:02:00Z", 1))
        .await
        .expect("insert c3");

    let (page2, total) = temp
        .db
        .query_connection_logs(
            Some("agent-test"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("end"),
            Some("desc"),
            1,
            1,
        )
        .await
        .expect("query paged");

    assert_eq!(total, 3);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].id, "c2");
}

#[tokio::test]
async fn connection_logs_cleanup_should_respect_rfc3339_boundaries() {
    use chrono::Utc;

    let temp = TempDb::new().await;
    let now = Utc::now();

    // 刚好超过 1 天：应被清理
    let old = (now - chrono::TimeDelta::days(1) - chrono::TimeDelta::minutes(1)).to_rfc3339();
    // 刚好不到 1 天：应保留
    let recent = (now - chrono::TimeDelta::days(1) + chrono::TimeDelta::minutes(1)).to_rfc3339();
    // 更老的数据
    let very_old = (now - chrono::TimeDelta::days(3)).to_rfc3339();

    temp.db
        .insert_connection_log(&make_log("old", "a.test", &old, 0))
        .await
        .expect("insert old");
    temp.db
        .insert_connection_log(&make_log("recent", "b.test", &recent, 0))
        .await
        .expect("insert recent");
    temp.db
        .insert_connection_log(&make_log("very_old", "c.test", &very_old, 0))
        .await
        .expect("insert very_old");

    let deleted = temp
        .db
        .cleanup_old_connection_logs(1)
        .await
        .expect("cleanup");
    assert_eq!(deleted, 2);

    let (items, total) = temp
        .db
        .query_connection_logs(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("end"),
            Some("asc"),
            10,
            0,
        )
        .await
        .expect("query after cleanup");

    assert_eq!(total, 1);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "recent");
}

#[tokio::test]
async fn cleanup_old_records_should_respect_age_only() {
    use chrono::Utc;

    let temp = TempDb::new().await;
    let now = Utc::now();
    let old = (now - chrono::TimeDelta::days(3)).to_rfc3339();
    let recent = (now - chrono::TimeDelta::hours(1)).to_rfc3339();

    // 老旧记录 -> 应删除（无论是否已同步）
    let old_record = make_connection_record("c1", "agent-a", &old, &old);
    temp.db
        .upsert_connection_record(&temp.db.pool, &old_record)
        .await
        .expect("insert old_record");

    // 较新记录 -> 应保留
    let recent_record = make_connection_record("c2", "agent-a", &recent, &recent);
    temp.db
        .upsert_connection_record(&temp.db.pool, &recent_record)
        .await
        .expect("insert recent_record");

    // 未同步但老旧 -> 也应删除
    let unsynced_old = make_connection_record("c3", "agent-a", &old, &now.to_rfc3339());
    temp.db
        .upsert_connection_record(&temp.db.pool, &unsynced_old)
        .await
        .expect("insert unsynced_old");

    let deleted = temp
        .db
        .cleanup_old_records(1)
        .await
        .expect("cleanup records");
    assert_eq!(deleted, 2);

    let rows = sqlx::query("SELECT id FROM connections WHERE agent_id = ?")
        .bind("agent-a")
        .fetch_all(&temp.db.pool)
        .await
        .expect("query remaining");
    let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    assert!(!ids.contains(&"c1".to_string()));
    assert!(ids.contains(&"c2".to_string()));
    assert!(!ids.contains(&"c3".to_string()));
}

#[tokio::test]
async fn cleanup_should_reject_non_positive_days() {
    let temp = TempDb::new().await;
    assert!(temp.db.cleanup_old_connection_logs(0).await.is_err());
    assert!(temp.db.cleanup_old_connection_logs(-1).await.is_err());
    assert!(temp.db.cleanup_old_records(0).await.is_err());
    assert!(temp.db.cleanup_old_records(-5).await.is_err());
}

#[tokio::test]
async fn malformed_timestamp_should_survive_cleanup() {
    let temp = TempDb::new().await;
    let log = make_log("bad-time", "a.test", "not-a-datetime", 1);
    temp.db.insert_connection_log(&log).await.expect("insert");
    let deleted = temp
        .db
        .cleanup_old_connection_logs(9999)
        .await
        .expect("cleanup");
    assert_eq!(deleted, 0, "malformed end should not be deleted silently");
}

#[tokio::test]
async fn cleanup_old_connection_logs_should_delete_synced_too() {
    use chrono::Utc;

    let temp = TempDb::new().await;
    let now = Utc::now();
    let old = (now - chrono::TimeDelta::days(3)).to_rfc3339();

    // 已同步且超期 -> 应被 Master 清理
    temp.db
        .insert_connection_log(&make_log("synced-old", "a.test", &old, 1))
        .await
        .expect("insert synced old");

    let deleted = temp
        .db
        .cleanup_old_connection_logs(1)
        .await
        .expect("cleanup");
    assert_eq!(
        deleted, 1,
        "cleanup_old_connection_logs should delete synced records too"
    );

    let (items, total) = temp
        .db
        .query_connection_logs(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("end"),
            Some("asc"),
            10,
            0,
        )
        .await
        .expect("query after cleanup");
    assert_eq!(total, 0);
    assert!(items.is_empty());
}

#[tokio::test]
async fn cleanup_old_unsynced_connection_logs_should_respect_synced_flag() {
    use chrono::Utc;

    let temp = TempDb::new().await;
    let now = Utc::now();
    let old = (now - chrono::TimeDelta::days(3)).to_rfc3339();

    // 未同步且超期 -> 应删除
    temp.db
        .insert_connection_log(&make_log("unsynced-old", "a.test", &old, 0))
        .await
        .expect("insert unsynced old");
    // 已同步且超期 -> 应保留（Agent 专用函数不清理已同步）
    temp.db
        .insert_connection_log(&make_log("synced-old", "b.test", &old, 1))
        .await
        .expect("insert synced old");

    let deleted = temp
        .db
        .cleanup_old_unsynced_connection_logs(1)
        .await
        .expect("cleanup unsynced");
    assert_eq!(deleted, 1, "should only delete unsynced records");

    let (items, total) = temp
        .db
        .query_connection_logs(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("end"),
            Some("asc"),
            10,
            0,
        )
        .await
        .expect("query after cleanup");
    assert_eq!(total, 1);
    assert_eq!(items[0].id, "synced-old");
}

#[tokio::test]
async fn replace_connections_for_agent_should_be_atomic() {
    let temp = TempDb::new().await;

    let old1 = make_connection_record(
        "c1",
        "agent-x",
        "2026-04-16T08:00:00Z",
        "2026-04-16T08:00:00Z",
    );
    let old2 = make_connection_record(
        "c2",
        "agent-x",
        "2026-04-16T08:01:00Z",
        "2026-04-16T08:01:00Z",
    );
    temp.db
        .upsert_connection_record(&temp.db.pool, &old1)
        .await
        .expect("insert old1");
    temp.db
        .upsert_connection_record(&temp.db.pool, &old2)
        .await
        .expect("insert old2");

    // 全量替换：只保留 c3
    let new1 = make_connection_record(
        "c3",
        "agent-x",
        "2026-04-16T09:00:00Z",
        "2026-04-16T09:00:00Z",
    );
    temp.db
        .replace_connections_for_agent("agent-x", &[new1])
        .await
        .expect("replace");

    let rows = sqlx::query("SELECT id FROM connections WHERE agent_id = ?")
        .bind("agent-x")
        .fetch_all(&temp.db.pool)
        .await
        .expect("query remaining");
    let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&"c3".to_string()));
}

#[tokio::test]
async fn batch_delete_records_should_work() {
    let temp = TempDb::new().await;

    let r1 = make_connection_record(
        "c1",
        "agent-d",
        "2026-04-16T08:00:00Z",
        "2026-04-16T08:00:00Z",
    );
    let r2 = make_connection_record(
        "c2",
        "agent-d",
        "2026-04-16T08:01:00Z",
        "2026-04-16T08:01:00Z",
    );
    let r3 = make_connection_record(
        "c3",
        "agent-d",
        "2026-04-16T08:02:00Z",
        "2026-04-16T08:02:00Z",
    );
    temp.db
        .upsert_connection_record(&temp.db.pool, &r1)
        .await
        .expect("insert r1");
    temp.db
        .upsert_connection_record(&temp.db.pool, &r2)
        .await
        .expect("insert r2");
    temp.db
        .upsert_connection_record(&temp.db.pool, &r3)
        .await
        .expect("insert r3");

    let deleted = temp
        .db
        .batch_delete_records(&[
            ("c1".to_string(), "agent-d".to_string()),
            ("c3".to_string(), "agent-d".to_string()),
        ])
        .await
        .expect("batch delete");
    assert_eq!(deleted, 2);

    let rows = sqlx::query("SELECT id FROM connections WHERE agent_id = ?")
        .bind("agent-d")
        .fetch_all(&temp.db.pool)
        .await
        .expect("query remaining");
    let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&"c2".to_string()));
}

#[tokio::test]
async fn cleanup_synced_connection_logs_should_delete_only_synced() {
    let temp = TempDb::new().await;

    temp.db
        .insert_connection_log(&make_log("synced", "a.test", "2026-04-16T08:00:00Z", 1))
        .await
        .expect("insert synced");
    temp.db
        .insert_connection_log(&make_log("unsynced", "b.test", "2026-04-16T08:01:00Z", 0))
        .await
        .expect("insert unsynced");

    let deleted = temp
        .db
        .cleanup_synced_connection_logs()
        .await
        .expect("cleanup synced");
    assert_eq!(deleted, 1);

    let (items, total) = temp
        .db
        .query_connection_logs(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("end"),
            Some("asc"),
            10,
            0,
        )
        .await
        .expect("query after cleanup");
    assert_eq!(total, 1);
    assert_eq!(items[0].id, "unsynced");
}

#[tokio::test]
async fn cleanup_stale_connections_should_respect_last_updated() {
    use chrono::Utc;

    let temp = TempDb::new().await;
    let now = Utc::now();
    let old = (now - chrono::TimeDelta::days(3)).to_rfc3339();
    let recent = (now - chrono::TimeDelta::hours(1)).to_rfc3339();

    let old_record = make_connection_record("c1", "agent-s", &old, &old);
    let recent_record = make_connection_record("c2", "agent-s", &recent, &recent);
    temp.db
        .upsert_connection_record(&temp.db.pool, &old_record)
        .await
        .expect("insert old");
    temp.db
        .upsert_connection_record(&temp.db.pool, &recent_record)
        .await
        .expect("insert recent");

    let deleted = temp
        .db
        .cleanup_stale_connections(1)
        .await
        .expect("cleanup stale");
    assert_eq!(deleted, 1);

    let rows = sqlx::query("SELECT id FROM connections WHERE agent_id = ?")
        .bind("agent-s")
        .fetch_all(&temp.db.pool)
        .await
        .expect("query remaining");
    let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&"c2".to_string()));
}
