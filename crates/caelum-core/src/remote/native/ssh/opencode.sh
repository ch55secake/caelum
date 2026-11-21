set -eu
PATH="$HOME/.opencode/bin:$HOME/.local/bin:$PATH"
reg="${XDG_STATE_HOME:-$HOME/.local/state}/opencode/service.json"
# Use the provider's registration, including its generated password, exclusively
# over the pinned SSH channel. Only the provider writes to its state directory.
registered() {
    [ -s "$reg" ] || return 1
    pid=$(sed -nE 's/.*"pid"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$reg")
    [ -n "$pid" ] && [ "$pid" -gt 1 ] && kill -0 "$pid" 2>/dev/null
}
if ! registered; then
    command -v opencode >/dev/null
    nohup opencode serve --service --hostname 127.0.0.1 --port 0 </dev/null >/dev/null 2>&1 &
    child=$!
    i=0
    until registered; do
        kill -0 "$child" 2>/dev/null && [ "$i" -lt 120 ] || exit 1
        i=$((i+1))
        sleep 0.2
    done
fi
cat "$reg"
