@echo off
cd /d "%~dp0"

@REM set RUST_LOG=warn
.\target\release\mihomo-tracker.exe agent ^
  --local-database ./db/agent.db ^
  --log-dir ./logs ^
  --master-url http://127.0.0.1:8051 ^
  --master-token YOUR_MASTER_TOKEN ^
  --agent-id agent_id_1 ^
  --data-retention-days 1 ^
  --log-retention-days 7 ^
  --mihomo-host 127.0.0.1 ^
  --mihomo-port 9097 ^
  --mihomo-token ""