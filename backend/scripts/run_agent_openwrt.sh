#!/bin/sh
#
# OpenWrt / ImmortalWrt 便捷启动脚本（临时后台运行）
# 用法: ./run_agent_openwrt.sh [start|stop|status]
#
# 注意：此脚本仅用于快速验证或临时运行，生产环境建议使用 procd init 脚本。
#

# -------------- 配置区（按需修改） --------------
AGENT_BIN="/usr/bin/mihomo-tracker"
WORK_DIR="/tmp/mihomo-tracker"
PID_FILE="$WORK_DIR/agent.pid"

# Master 连接信息
MASTER_URL="http://192.168.1.100:8051"
MASTER_TOKEN="YOUR_MASTER_TOKEN"
AGENT_ID="openwrt-router"

# Mihomo 连接信息（通常在本机）
MIHOMO_HOST="127.0.0.1"
MIHOMO_PORT="9097"
MIHOMO_TOKEN=""

# 存储与日志（默认放 /tmp，保护路由器闪存）
LOCAL_DB="$WORK_DIR/agent.db"
LOG_DIR="$WORK_DIR/logs"

# 保留策略
DATA_RETENTION_DAYS="1"
LOG_RETENTION_DAYS="7" # 离线日志保留时间
SYNC_INTERVAL="60"
# -------------- 配置区结束 --------------

# 检测进程是否在运行（不依赖 PID 文件）
# 用 pgrep 匹配进程名，排除 shell 脚本自身和 grep 进程
is_running() {
    # ps: CMD 列包含 /usr/bin/mihomo-tracker 且是 agent 模式，排除 .sh 脚本
    ps -w | grep -v "grep" | grep -v "\.sh" | grep -q "/usr/bin/mihomo-tracker.*agent"
}

# 获取当前 Agent PID
get_pid() {
    ps -w | grep -v "grep" | grep -v "\.sh" | grep "/usr/bin/mihomo-tracker.*agent" | awk '{print $1}' | head -1
}

start_agent() {
    if is_running; then
        PID=$(get_pid)
        echo "Agent already running (PID: $PID)"
        echo "Logs: $LOG_DIR"
        echo "Database: $LOCAL_DB"
        exit 0
    fi

    mkdir -p "$WORK_DIR" "$LOG_DIR"

    echo "Starting mihomo-tracker agent..."
    # 不用 nohup，直接后台运行
    # trap '' HUP: 子 shell 忽略 SIGHUP，SSH 断开后进程不被杀死
    # exec: 替换子 shell 为实际进程，PID 保持一致
    # RUST_LOG=warn: 只记录 warn/error，减少日志量
    (
        trap '' HUP
        RUST_LOG=warn
        export RUST_LOG
        exec "$AGENT_BIN" agent \
            --local-database "$LOCAL_DB" \
            --log-dir "$LOG_DIR" \
            --master-url "$MASTER_URL" \
            --master-token "$MASTER_TOKEN" \
            --agent-id "$AGENT_ID" \
            --mihomo-host "$MIHOMO_HOST" \
            --mihomo-port "$MIHOMO_PORT" \
            --mihomo-token "$MIHOMO_TOKEN" \
            --data-retention-days "$DATA_RETENTION_DAYS" \
            --log-retention-days "$LOG_RETENTION_DAYS" \
            --sync-interval "$SYNC_INTERVAL"
    ) >> "$WORK_DIR/agent.out" 2>&1 &

    # 等待进程启动（最多 3 秒）
    sleep 1
    if is_running; then
        PID=$(get_pid)
        echo $PID > "$PID_FILE"
        echo "Agent started (PID: $PID)"
        echo "Work dir: $WORK_DIR"
        echo "Database: $LOCAL_DB ($(ls -lh "$LOCAL_DB" 2>/dev/null | awk '{print $5}'))"
        echo "Log dir: $LOG_DIR ($(du -sh "$LOG_DIR" 2>/dev/null | awk '{print $1}'))"
    else
        echo "Agent failed to start. Check $WORK_DIR/agent.out for details."
        exit 1
    fi
}

stop_agent() {
    if ! is_running; then
        echo "Agent is not running"
        rm -f "$PID_FILE"
        exit 0
    fi

    PID=$(get_pid)
    echo "Stopping agent (PID: $PID)..."
    kill "$PID" 2>/dev/null

    # 等待进程退出（最多 5 秒）
    for i in 1 2 3 4 5; do
        if ! is_running; then
            break
        fi
        sleep 1
    done

    if is_running; then
        echo "Agent did not stop gracefully, forcing..."
        kill -9 "$PID" 2>/dev/null
    fi

    rm -f "$PID_FILE"
    echo "Agent stopped"
}

status_agent() {
    if is_running; then
        PID=$(get_pid)
        DB_SIZE=$(ls -lh "$LOCAL_DB" 2>/dev/null | awk '{print $5}')
        LOG_SIZE=$(du -sh "$LOG_DIR" 2>/dev/null | awk '{print $1}')
        echo "Agent is running (PID: $PID)"
        echo "Work dir: $WORK_DIR"
        echo "Database: $LOCAL_DB ($DB_SIZE)"
        echo "Log dir: $LOG_DIR ($LOG_SIZE)"
        echo "Master: $MASTER_URL"
    else
        echo "Agent is not running"
        [ -f "$PID_FILE" ] && rm -f "$PID_FILE"
    fi
}

case "${1:-start}" in
    start)
        start_agent
        ;;
    stop)
        stop_agent
        ;;
    restart)
        stop_agent
        sleep 1
        start_agent
        ;;
    status)
        status_agent
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status}"
        exit 1
        ;;
esac
