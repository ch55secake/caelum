set -eu
PATH="$HOME/.local/bin:$PATH"
port=${CAELUM_CODEX_PORT:-4500}
case "$port" in ''|*[!0-9]*) exit 1;; esac
ready() { curl -fsS --noproxy '*' --max-time 1 "http://127.0.0.1:$port/readyz" >/dev/null 2>&1; }
if ! ready; then
    command -v codex >/dev/null
    nohup codex app-server --listen "ws://127.0.0.1:$port" </dev/null >/dev/null 2>&1 &
    pid=$!
    i=0
    until ready; do
        kill -0 "$pid" 2>/dev/null && [ "$i" -lt 50 ] || exit 1
        i=$((i+1))
        sleep 0.2
    done
fi
printf '%s\n' "$port"
