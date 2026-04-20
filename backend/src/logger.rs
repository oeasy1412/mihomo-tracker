use std::fs;
use std::path::Path;
use std::time::Duration;
use tokio::time::interval;
use tracing_subscriber::{
    fmt::{self},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_tracing(log_dir: &str) -> Result<(), String> {
    let log_path = Path::new(log_dir);
    if let Err(e) = fs::create_dir_all(log_path) {
        return Err(format!("无法创建日志目录 {}: {}", log_dir, e));
    }

    let file_appender = tracing_appender::rolling::daily(log_dir, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // 保留 _guard 防止被 Drop，从而保持后台写入线程存活
    // 使用 Box::leak 将其静态化（简单且符合 CLI 长期运行场景）
    let _ = Box::leak(Box::new(_guard));

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer_file = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .json()
        .with_span_list(false)
        .with_current_span(false);

    let fmt_layer_stdout = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .pretty();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer_file)
        .with(fmt_layer_stdout)
        .init();

    tracing::info!(target: "backend::logger", "Logging initialized, log_dir: {}", log_dir);

    // 启动日志文件保留清理任务（每天执行一次）
    let log_dir_owned = log_dir.to_string();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(24 * 60 * 60));
        ticker.tick().await; // 首次等待一个周期
        loop {
            let log_dir = log_dir_owned.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || {
                cleanup_old_log_files(&log_dir, 7);
            })
            .await
            {
                tracing::error!(target: "backend::logger", "日志清理任务执行失败: {:?}", e);
            }
            ticker.tick().await;
        }
    });

    Ok(())
}

fn cleanup_old_log_files(log_dir: &str, days_to_keep: u64) {
    let cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(days_to_keep * 24 * 60 * 60);

    let entries = match fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(target: "backend::logger", "无法读取日志目录 {}: {}", log_dir, e);
            return;
        }
    };

    for entry_result in entries {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(target: "backend::logger", "无法读取目录项: {}", e);
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file() || !is_managed_log_file(&path) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(target: "backend::logger", "无法获取文件元数据 {}: {}", path.display(), e);
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(target: "backend::logger", "无法获取修改时间 {}: {}", path.display(), e);
                continue;
            }
        };
        if modified < cutoff {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!(target: "backend::logger", "删除旧日志文件失败 {}: {}", path.display(), e);
            } else {
                tracing::info!(target: "backend::logger", "Removed old log file {}", path.display());
            }
        }
    }
}

fn is_managed_log_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("app.log"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_managed_log_file;

    #[test]
    fn log_cleanup_only_targets_app_log_files() {
        assert!(is_managed_log_file(Path::new("./logs/app.log")));
        assert!(is_managed_log_file(Path::new("./logs/app.log.2026-04-16")));
        assert!(!is_managed_log_file(Path::new("./logs/system.log")));
        assert!(!is_managed_log_file(Path::new("./logs/master.db")));
    }
}
