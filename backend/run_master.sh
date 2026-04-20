#!/bin/bash

cd "$(dirname "$0")"

./target/release/mihomo-tracker master \
  --database ./db/master.db \
  --log-dir ./logs \
  --log-retention-days 30 \
  --listen-host 0.0.0.0 \
  --listen-port 8051 \
  --api-token YOUR_MASTER_TOKEN
