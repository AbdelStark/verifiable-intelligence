#!/usr/bin/env sh
set -eu

VI_BIND_ADDR="${VI_BIND_ADDR:-0.0.0.0:8000}"
VI_HEALTHZ_BIND_ADDR="${VI_HEALTHZ_BIND_ADDR:-$VI_BIND_ADDR}"
VI_MAX_NUM_SEQS="${VI_MAX_NUM_SEQS:-8}"
VI_LOG_LEVEL="${VI_LOG_LEVEL:-info}"
MODEL_DIR="${MODEL_DIR:-/weights}"
MODEL_ID="${MODEL_ID:-llama-3.1-8b-w8a8}"
COMMITLLM_SHORT="${COMMITLLM_SHORT:-25541e83}"
VI_CHECKPOINT_HASH="${VI_CHECKPOINT_HASH:-sha256:0000000000000000000000000000000000000000000000000000000000000000}"
VI_KEY_HASH="${VI_KEY_HASH:-sha256:0000000000000000000000000000000000000000000000000000000000000000}"
SERVER_PID=""
HEALTHZ_PID=""

export COMMITLLM_SHORT MODEL_ID VI_CHECKPOINT_HASH VI_KEY_HASH VI_LOG_LEVEL

script_dir() {
    dirname "$0"
}

bind_host() {
    printf '%s' "${1%:*}"
}

bind_port() {
    printf '%s' "${1##*:}"
}

health_check_host() {
    host="$(bind_host "$1")"
    if [ "$host" = "0.0.0.0" ] || [ "$host" = "" ]; then
        printf '%s' "127.0.0.1"
    else
        printf '%s' "$host"
    fi
}

log_event() {
    python3 - "$@" <<'PY'
import json
import os
import sys
import time

event = sys.argv[1]
payload = {
    "event": event,
    "level": os.environ.get("VI_LOG_LEVEL", "info"),
    "timestamp_unix_ms": int(time.time() * 1000),
}
for item in sys.argv[2:]:
    key, value = item.split("=", 1)
    if key in {"exit_code", "max_num_seqs", "pid", "port"}:
        try:
            payload[key] = int(value)
            continue
        except ValueError:
            pass
    payload[key] = value

print(json.dumps(payload, separators=(",", ":"), sort_keys=True), file=sys.stderr, flush=True)
PY
}

wait_for_healthz() {
    url="$1"
    python3 - "$url" <<'PY'
import os
import sys
import time
import urllib.request

url = sys.argv[1]
deadline = time.time() + float(os.environ.get("VI_HEALTHZ_TIMEOUT_S", "10"))
last_error = None
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url, timeout=0.5) as response:
            if response.status == 200:
                sys.exit(0)
    except Exception as error:  # noqa: BLE001 - command-line readiness probe.
        last_error = error
    time.sleep(0.1)

print(f"healthz did not become ready at {url}: {last_error}", file=sys.stderr)
sys.exit(1)
PY
}

start_healthz() {
    healthz_script="${VI_HEALTHZ_SCRIPT:-/healthz.py}"
    if [ ! -f "$healthz_script" ]; then
        healthz_script="$(script_dir)/healthz.py"
    fi
    python3 "$healthz_script" --bind "$VI_HEALTHZ_BIND_ADDR" &
    HEALTHZ_PID="$!"
}

start_provider() {
    if [ "${VI_PROVIDER_STUB:-0}" = "1" ]; then
        python3 - <<'PY' &
import signal
import sys
import time

def stop(_signum, _frame):
    sys.exit(0)

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
while True:
    time.sleep(1)
PY
        SERVER_PID="$!"
        return
    fi

    host="$(bind_host "$VI_BIND_ADDR")"
    port="$(bind_port "$VI_BIND_ADDR")"
    vllm serve "$MODEL_DIR" \
        --host "$host" \
        --port "$port" \
        --max-num-seqs "$VI_MAX_NUM_SEQS" \
        ${VI_VLLM_EXTRA_ARGS:-} &
    SERVER_PID="$!"
}

stop_child() {
    pid="$1"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
}

shutdown() {
    signal="${1:-SIGTERM}"
    log_event provider.shutdown signal="$signal" exit_code=0
    stop_child "$SERVER_PID"
    stop_child "$HEALTHZ_PID"
    exit 0
}

trap 'shutdown SIGTERM' TERM
trap 'shutdown SIGINT' INT

log_event provider.boot \
    commitllm_pin="$COMMITLLM_SHORT" \
    model_id="$MODEL_ID" \
    checkpoint_hash="$VI_CHECKPOINT_HASH" \
    key_hash="$VI_KEY_HASH" \
    bind_addr="$VI_BIND_ADDR" \
    max_num_seqs="$VI_MAX_NUM_SEQS"

start_healthz
start_provider

health_url="http://$(health_check_host "$VI_HEALTHZ_BIND_ADDR"):$(bind_port "$VI_HEALTHZ_BIND_ADDR")/healthz"
wait_for_healthz "$health_url"
log_event provider.ready \
    healthz="$health_url" \
    pid="$SERVER_PID" \
    port="$(bind_port "$VI_BIND_ADDR")"

if [ "${VI_PROVIDER_STUB_EXIT_AFTER_READY:-0}" = "1" ]; then
    shutdown stub_complete
fi

set +e
wait "$SERVER_PID"
exit_code="$?"
set -e
stop_child "$HEALTHZ_PID"
log_event provider.shutdown reason=provider_exit exit_code="$exit_code"
exit "$exit_code"
