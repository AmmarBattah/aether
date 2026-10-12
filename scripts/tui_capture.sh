#!/usr/bin/env bash
# Drive the TUI inside tmux against the mock provider and capture frames as
# text (plain and with ANSI colours) for review.
#
#   scripts/tui_capture.sh [binary] [out-dir] [theme] [width] [height] [ascii]
#
# theme is any name --theme takes: auto (default), dark, light, mono, a
# built-in such as nord or gruvbox-light; frames land in out-dir. Pass ascii=1
# as the sixth argument for interactive ASCII decoration (Unicode by default).
# Set SMOKE=1 for a bounded lane instead of the full ~40-frame session: real
# startup, composer, the Ctrl+G board, the shortcuts panel and quit only.
# Meant for extra width lanes (e.g. responsive breakpoints) where running the
# full session at every width would not be worth its cost.
# Set THEME_LANE=1 for the colour lane instead: markdown, an explored batch,
# an edit, a shell call caught running, a board with a running grandchild,
# then the /theme picker.
# Set MODE_LANE=1 for an external compact mode, hidden tools, details and /mode.
#
# Temporary state goes under $TMPDIR (see scripts/lib.sh's tmp_base); keep it
# short, as the daemon's unix socket lives there.
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/lib.sh
BIN=$(resolve_path "${1:-target/debug/aether}")
OUT=${2:-target/tui-captures}
THEME=${3:-auto}
WIDTH=${4:-160}
HEIGHT=${5:-44}
ASCII=${6:-0}
SMOKE=${SMOKE:-0}
THEME_LANE=${THEME_LANE:-0}
MODE_LANE=${MODE_LANE:-0}
VIEW_MODE=standard
case "$THEME" in
  dusk) THEME=dark ;;
  dawn) THEME=light ;;
esac
if [[ ! "$THEME" =~ ^[a-z0-9][a-z0-9_-]*$ ]]; then
  echo "unknown theme $THEME: use auto, dark, light, mono or a theme name" >&2
  exit 2
fi
# The palette tmux answers the TUI's OSC 10/11 probe with, so themes paint
# their tint and pulse; each family on the ground its users would run it on.
# Auto gets no answer: the unknown-terminal fallback.
case "$THEME" in
  auto) PANE_STYLE= ;;
  nord-light) PANE_STYLE='fg=#2e3440,bg=#eceff4' ;;
  nord*) PANE_STYLE='fg=#d8dee9,bg=#2e3440' ;;
  catppuccin-light | catppuccin-latte) PANE_STYLE='fg=#4c4f69,bg=#eff1f5' ;;
  catppuccin*) PANE_STYLE='fg=#cdd6f4,bg=#1e1e2e' ;;
  gruvbox-light) PANE_STYLE='fg=#3c3836,bg=#fbf1c7' ;;
  gruvbox*) PANE_STYLE='fg=#ebdbb2,bg=#282828' ;;
  tokyo-night-light | tokyo-night-day) PANE_STYLE='fg=#3760bf,bg=#e1e2e7' ;;
  tokyo-night*) PANE_STYLE='fg=#c0caf5,bg=#1a1b26' ;;
  light | *-light) PANE_STYLE='fg=#1f2328,bg=#ffffff' ;;
  *) PANE_STYLE='fg=#e6edf3,bg=#0d1117' ;;
esac
mkdir -p "$OUT"
ROOT=$(mktemp -d "$(tmp_base)/tui-XXXXXX")
export AETHER_CONFIG_DIR="$ROOT/config" AETHER_DATA_DIR="$ROOT/data" AETHER_SOCKET="$ROOT/aetherd.sock"
# sun_path holds 108 bytes on Linux and 104 on macOS.
if [ "${#AETHER_SOCKET}" -ge 100 ]; then
  echo "socket path $AETHER_SOCKET is too long for a unix socket; set a shorter TMPDIR" >&2
  exit 1
fi
mkdir -p "$AETHER_CONFIG_DIR" "$AETHER_DATA_DIR" "$ROOT/workspace"
printf '# sample\n' > "$ROOT/workspace/README.md"
if [ "$MODE_LANE" = 1 ]; then
  mkdir -p "$AETHER_CONFIG_DIR/modes"
  cat > "$AETHER_CONFIG_DIR/modes/brief.toml" <<'MODE'
version = 1
density = "compact"
show = ["user_text", "assistant_text", "delegation", "agent_message", "completion", "status"]
MODE
  VIEW_MODE=brief
fi
SESSION="aether-capture-$$"
# A private tmux server, started without the operator's config: the default
# one is shared with concurrent CI runs and the operator's own sessions, and
# its panes inherit the environment of whichever client started it. Panes stay
# after aether exits, so an exit is caught with its last screen, not lost.
TMUX_SOCKET="$ROOT/tmux.sock"
{
  printf 'set-option -g remain-on-exit on\n'
  if [ -n "$PANE_STYLE" ]; then printf 'set-option -g window-style "%s"\n' "$PANE_STYLE"; fi
} > "$ROOT/tmux.conf"
unset TMUX
tm() { tmux -S "$TMUX_SOCKET" -f "$ROOT/tmux.conf" "$@"; }
MOCK=
DAEMON=
# On failure, keep what explains it beside the frames: the pane as aether left
# it, and the daemon, startup and mock logs. Best effort: any may not exist.
keep_failure() {
  local dir="$OUT/failure"
  mkdir -p "$dir"
  tm display-message -p -t "$SESSION" \
    'dead=#{pane_dead} status=#{pane_dead_status} signal=#{pane_dead_signal}' \
    > "$dir/pane-state.txt" 2>&1 || true
  tm capture-pane -t "$SESSION" -p -S - > "$dir/pane.txt" 2>&1 || true
  tm capture-pane -t "$SESSION" -p -e -N > "$dir/pane.ansi" 2>&1 || true
  cp -R "$AETHER_DATA_DIR/logs" "$dir/logs" 2>/dev/null || true
  cp "$ROOT/aetherd.startup.log" "$ROOT/mock.log" "$dir/" 2>/dev/null || true
  echo "capture failed: pane and logs kept in $dir" >&2
}
cleanup() {
  local status=$?
  if [ "$status" != 0 ]; then
    keep_failure
  fi
  tm kill-server 2>/dev/null || true
  if [ -n "$DAEMON" ]; then stop_test_child "$DAEMON"; fi
  if [ -n "$MOCK" ]; then stop_test_child "$MOCK"; fi
  echo "captures in $OUT (env in $ROOT)"
  exit "$status"
}
trap cleanup EXIT

# Port 0: the mock binds a free port and names it. A port picked here could be
# taken by a concurrent run's mock, whose provider this TUI would then use.
# The log exists before the mock starts, so the poll below never reads a gap.
: > "$ROOT/mock.log"
python3 scripts/mock_openai.py 0 > "$ROOT/mock.log" 2>&1 &
MOCK=$!
PORT=
for ((attempt = 0; attempt < 50; attempt++)); do
  PORT=$(sed -n 's|^mock openai server on http://127.0.0.1:\([0-9]*\)/v1$|\1|p' "$ROOT/mock.log")
  if [ -n "$PORT" ] || ! kill -0 "$MOCK" 2>/dev/null; then
    break
  fi
  sleep 0.1
done
if [ -z "$PORT" ]; then
  echo "mock provider did not start" >&2
  exit 1
fi
cat > "$AETHER_CONFIG_DIR/providers.toml" <<EOF
[providers.mock]
name = "Mock (local)"
protocol = "openai-chat-completions"
base_url = "http://127.0.0.1:$PORT/v1"
[providers.mock.auth]
none = true
[[providers.mock.models]]
id = "mock-1"
name = "Mock model"
[providers.mock.models.capabilities]
context_window = 32768
max_output_tokens = 4096
tools = true
EOF
printf '[model]\ndefault = "mock/mock-1"\n' > "$AETHER_CONFIG_DIR/config.toml"

# Start our own foreground child before the TUI connects. Its client-side
# autostart gives up after 10s; a cold capture daemon gets a bounded 30s wait
# and a diagnostic from its startup log instead.
"$BIN" daemon --foreground > "$ROOT/aetherd.startup.log" 2>&1 &
DAEMON=$!
wait_for_test_daemon "$BIN" "$DAEMON" "$ROOT/aetherd.startup.log" 30

# Prints how aether ended ("status 0", "signal 9"), or nothing while it runs.
# Before tmux 3.7 a pane is dead once its terminal closes, with no status yet.
pane_end() {
  local state dead status signal server attempt
  for ((attempt = 0; attempt < 50; attempt++)); do
    state=$(tm display-message -p -t "$SESSION" \
      '#{pane_dead}:#{pane_dead_status}:#{pane_dead_signal}:#{pid}') || return 1
    IFS=: read -r dead status signal server <<< "$state"
    case "$dead:$server" in
      0:[1-9]*) return 0 ;;
      1:[1-9]*) ;;
      *) echo "tmux gave an unreadable pane state '$state'" >&2; return 1 ;;
    esac
    if [ -n "$status" ]; then
      echo "status $status"
      return 0
    fi
    if [ -n "$signal" ]; then
      echo "signal $signal"
      return 0
    fi
    # Before 3.6 a tmux built with utempter can lose the SIGCHLD of an exit that
    # follows the closed terminal within a millisecond. This one makes it look.
    kill -CHLD "$server" || return 1
    sleep 0.1
  done
  echo "no exit status known to tmux 5s after its terminal closed"
}
# Fails once aether has exited: every frame after that would be its corpse.
alive() {
  local ended
  ended=$(pane_end) || return 1
  if [ -n "$ended" ]; then
    echo "aether exited with $ended during the capture" >&2
    return 1
  fi
}
grab() { # name [capture-pane flags]
  tm capture-pane -t "$SESSION" -p "${@:2}" > "$OUT/$1.txt"
  tm capture-pane -t "$SESSION" -p -e -N "${@:2}" > "$OUT/$1.ansi"
  echo "captured $1"
}
snap() { # name [settle-seconds]
  sleep "${2:-1}"
  alive
  grab "$1"
}
# Wait for what a lone Escape closes before the next key: aether reads an Escape
# and a key that arrive together as Alt+key, and the panel stays open.
keys() { tm send-keys -t "$SESSION" "$@"; }
# Patterns are bash regexes over the plain pane. The optional third argument
# is the deadline in tenths of a second.
wait_for_text() {
  local pane attempt
  for ((attempt = 0; attempt < ${3:-100}; attempt++)); do
    alive || return 1
    pane=$(tm capture-pane -t "$SESSION" -p)
    if [[ "$pane" =~ $1 ]]; then
      return 0
    fi
    sleep 0.1
  done
  snap "timeout-$2" 0
  echo "timed out waiting for $2" >&2
  return 1
}
wait_for_absence() {
  local pane attempt
  for ((attempt = 0; attempt < ${3:-100}; attempt++)); do
    alive || return 1
    pane=$(tm capture-pane -t "$SESSION" -p)
    if [[ ! "$pane" =~ $1 ]]; then
      return 0
    fi
    sleep 0.1
  done
  snap "timeout-$2" 0
  echo "timed out waiting for $2 to clear" >&2
  return 1
}
# The status row (and its "esc to interrupt") shows only while the viewed
# agent is busy.
wait_idle() { wait_for_absence "esc to interrupt" "$1" "${2:-150}"; }
# Ctrl+C on an empty composer quits at once (SPEC §4.2) and, once the terminal
# is restored, leaves the detach message in the pane, kept by remain-on-exit.
quit_and_capture() {
  local ended='' attempt
  keys C-c
  for ((attempt = 0; attempt < 50; attempt++)); do
    ended=$(pane_end) || return 1
    if [ -n "$ended" ]; then
      break
    fi
    sleep 0.1
  done
  # With history: tmux's "Pane is dead" row scrolls the message's row off.
  grab 15-quit -S -
  if [ -z "$ended" ]; then
    echo "Ctrl+C on an empty composer did not quit" >&2
    return 1
  fi
  if [ "$ended" != "status 0" ]; then
    echo "aether quit with $ended" >&2
    return 1
  fi
  # -J joins the message's rows where a narrow pane wrapped it.
  if ! tm capture-pane -t "$SESSION" -p -J -S - |
    grep -q "Agents keep running in the daemon.*Reattach with: aether"; then
    echo "the pane has no 'Agents keep running … Reattach with: aether' line after quit" >&2
    return 1
  fi
}

# Approvals stay on (--no-yolo) so the mock's web_fetch asks for one. COLORTERM is set
# here: tmux sets it only from 3.6 on, and the frames' colour depth must not vary.
tm new-session -d -s "$SESSION" -x "$WIDTH" -y "$HEIGHT" -c "$ROOT/workspace" \
  "env AETHER_CONFIG_DIR=$AETHER_CONFIG_DIR AETHER_DATA_DIR=$AETHER_DATA_DIR AETHER_SOCKET=$AETHER_SOCKET AETHER_ASCII=$ASCII AETHER_VIEW_MODE=$VIEW_MODE TERM=xterm-256color COLORTERM=truecolor $BIN tui --theme $THEME --no-yolo"
wait_for_text "Message assistant" startup-agent 200
# A transient connection notice would cover the hint row's key hints.
wait_for_absence "connected" connection-notice
snap 01-startup 0.5
keys "hello" Enter
wait_for_text "I am a mock model" hello
wait_idle hello
snap 02-hello
if [ "$MODE_LANE" = 1 ]; then
  keys "please list the files in the workspace" Enter
  wait_for_text "workspace has" mode-tool-response
  wait_idle mode-tool-response
  snap 03-mode-filtered
  keys C-t
  wait_for_text "Ran ls" mode-details
  snap 04-mode-details
  keys C-t
  keys "/mode" Enter
  wait_for_text "View modes" mode-picker
  snap 05-mode-picker
  keys Escape
  wait_for_text "Message assistant" mode-picker-closed
  quit_and_capture
  exit 0
fi
if [ "$THEME_LANE" = 1 ]; then
  keys "show me some markdown" Enter
  wait_for_text "hello from aether" markdown
  wait_idle markdown
  snap 02a-markdown
  keys "explore the workspace" Enter
  wait_for_text "Explored" explored
  wait_idle explored
  keys "write a file" Enter
  wait_for_text "hello.txt" edit
  wait_idle edit
  snap 03-batch
  keys "run the slow command" Enter
  wait_for_text "sleep 6" running-tool
  snap 03a-running-tool 1.5
  wait_idle running-tool
  # The board docks at this width; the grandchild's slow command keeps the
  # tree running for a few seconds after the parent hands off.
  keys "/agent" Enter
  wait_for_text "Agents +[0-9]" board
  keys Escape
  wait_for_absence "(⇧|shift\+)x" board-unfocused
  keys "delegate deeply" Enter
  wait_for_text "Handed the listing" handoff 300
  wait_for_text "agents · [0-9]+ running" grandchild 300
  snap 04-board-running 1.5
  wait_for_absence "agents · [0-9]+ running" settled 600
  # The child's result starts one more parent turn; Ctrl+C would interrupt it.
  sleep 1
  wait_idle settled-inbox 300
  snap 04a-board-settled
  keys "/theme" Enter
  wait_for_text "Theme +now" theme-picker
  snap 05-theme-picker
  keys Escape
  wait_for_text "Message assistant" theme-picker-closed
  quit_and_capture
  exit 0
fi
if [ "$SMOKE" = 1 ]; then
  # Bounded lane: an unsent draft (composer wrap/clip at this width), the
  # board (overlay below 100 columns, docked from 100) and the shortcuts
  # panel - the real-terminal facts a width lane needs beyond startup.
  keys "Please review the documentation and report the findings back here"
  snap 02d-draft
  keys C-c
  wait_for_text "Message assistant" draft-cleared
  keys C-g
  wait_for_text "Agents +[0-9]" board
  snap 03-agents
  keys Escape
  wait_for_absence "(⇧|shift\+)x" board-unfocused
  keys "?"
  wait_for_text "Keyboard shortcuts" shortcuts
  snap 05-shortcuts
  keys Escape
  wait_for_absence "Keyboard shortcuts" shortcuts-closed
  quit_and_capture
  exit 0
fi
# Printables reach the draft once it is non-empty; `q` never quits.
keys "qieomyna?"
wait_for_text "qieomyna\?" composer-printables
snap 02i-composer-printables
keys C-c
# Feed the terminal's enhanced Shift+Enter sequence through the real input reader.
keys "First draft line"
tm send-keys -t "$SESSION" -H 1b 5b 31 33 3b 32 75
keys "Second draft line"
snap 02e-shift-enter
keys C-c
# A long draft wraps and grows until the composer reaches its height cap.
# Bracketed paste keeps this a draft instead of submitting it; it stays under
# the 1,000 chars that would collapse it into a [Pasted …] placeholder.
python3 - "$ROOT/draft.txt" <<'PYDRAFT'
import pathlib, sys
pathlib.Path(sys.argv[1]).write_text("Please review the documentation and report the findings. " * 16 + "DRAFT END")
PYDRAFT
tm load-buffer "$ROOT/draft.txt"
tm paste-buffer -p -t "$SESSION"
snap 02c-wrapped-draft
tm resize-window -t "$SESSION" -x 80 -y 24
snap 02d-narrow-draft
tm resize-window -t "$SESSION" -x "$WIDTH" -y "$HEIGHT"
keys C-c
wait_for_text "Message assistant" draft-cleared
keys "show me some markdown" Enter
wait_for_text "hello from aether" markdown
wait_idle markdown
snap 02a-markdown
# Tab while the turn runs queues for after it; Esc then interrupts the turn,
# which sends the queued message.
keys "count slowly please" Enter
wait_for_text "Counting slowly: 1," slow-reply
keys "hello again"
keys Tab
wait_for_text "queued for after this turn" queued
snap 27-queued 0.5
keys Escape
wait_for_text "Interrupted" interrupted
wait_idle queued-sent
snap 02b-interrupted
keys "please list the files in the workspace" Enter
wait_for_text "Ran ls" tool-call
wait_idle tool-call
snap 03-tool-call
keys "run the failing command" Enter
wait_for_text "Ran cat missing-notes.txt" failed-tool
wait_idle failed-tool
snap 03a-failed-tool
keys "delegate the listing to a child" Enter
wait_for_text "finished" delegate 300
wait_idle delegate 300
snap 04-delegate 2
# The child's completion reaches the parent's inbox and starts another turn.
wait_idle delegate-inbox 300
# A tool output too big for the context becomes an artifact; Ctrl+T shows
# every cell in full, and /artifacts lists and opens it.
keys "give me a big output please" Enter
wait_for_text "full output saved" big-output 300
wait_idle big-output
keys C-t
wait_for_text "exit 0" details
snap 22-details
keys C-t
keys C-End
keys "/artifacts" Enter
wait_for_text "Artifacts ·" artifacts
snap 23-artifacts
keys Enter
wait_for_text "aether artifact line" artifact
snap 24-artifact
keys Escape
wait_for_text "Artifacts ·" artifact-pager-closed
keys Escape
wait_for_text "Message assistant" artifacts-closed
keys "?"
wait_for_text "Keyboard shortcuts" shortcuts
snap 05-shortcuts
keys Escape
wait_for_absence "Keyboard shortcuts" shortcuts-closed
keys C-r
keys "mark"
wait_for_text "search history mark" history-search
snap 25-history-search
keys Escape
wait_for_text "Message assistant" history-search-closed
# The palette is the other surface that explains the product, so it is
# captured like the shortcuts rather than driven by hand.
keys "/"
wait_for_text "enter run" palette
snap 21-palette
keys BSpace
wait_for_text "Message assistant" palette-closed
keys "/doctor" Enter
wait_for_text "providers +[0-9]+ of [0-9]+" doctor
snap 05a-doctor
keys Escape
wait_for_text "Message assistant" doctor-closed
keys "/report" Enter
wait_for_text "Report ·" report
snap 09b-report
keys Escape
wait_for_text "Message assistant" report-closed
keys C-f
keys "workspace"
wait_for_text "find workspace" find
snap 09a-find
keys Escape
wait_for_absence "find workspace" find-closed
keys C-End
keys "/models" Enter
wait_for_text "Model and effort" models
snap 10-models
keys Escape
wait_for_text "Message assistant" models-closed
keys "/usage" Enter
wait_for_text "Usage" usage
snap 11a-usage
keys Escape
wait_for_text "Message assistant" usage-closed
# Timers use the same daemon and refresh the open view after each action.
"$BIN" agents --json > "$ROOT/agents.json"
SCHEDULE_TARGET=$(python3 - "$ROOT/agents.json" <<'PYTARGET'
import json, sys
agents = json.load(open(sys.argv[1]))
print(next(agent["id"] for agent in agents if agent.get("parent") is None))
PYTARGET
)
"$BIN" schedule create "Documentation timer" --target "$SCHEDULE_TARGET" \
  --every 3600 --max-deliveries 3 --payload "Read the documentation" > "$ROOT/schedule.txt"
keys "/schedules" Enter
wait_for_text "Documentation timer" schedules
snap 11b-schedules 1.5
keys p
wait_for_text "Documentation timer.*paused" paused
snap 11c-schedule-paused 1.5
keys p
wait_for_text "Documentation timer.*active" resumed
snap 11d-schedule-resumed 1.5
keys c
wait_for_text "c again to cancel" confirmation
snap 11e-schedule-confirm
keys c
wait_for_text "Documentation timer.*cancelled" cancelled
snap 11f-schedule-cancelled 1.5
keys Escape
wait_for_text "Message assistant" schedules-closed
keys "/settings" Enter
wait_for_text "Settings" settings
snap 12-settings
keys Escape
wait_for_text "Message assistant" settings-closed
# /model's effort column (it replaced /effort).
keys "/model" Enter
wait_for_text "Model and effort" model
keys Right
snap 13-effort
keys Escape
wait_for_text "Message assistant" model-closed
# The board: /agent opens or focuses it, Enter on a child views that child,
# Alt+Up returns to the parent, Ctrl+G focuses the open board, Ctrl+B hides it.
keys "/agent" Enter
wait_for_text "Agents +[0-9]" board
snap 03-agents
keys Down
keys Enter
wait_for_text "‹ assistant" child
snap 03b-child
keys M-Up
wait_for_absence "‹ assistant" parent
keys C-g
# Only a focused board's footer names the lifecycle keys, in any width or glyph set.
wait_for_text "(⇧|shift\+)x" board-focused
keys C-b
wait_for_absence "Agents +[0-9]" board-hidden
snap 12-board-hidden
# Approvals last: the mock's follow-up turns after one can spawn more work.
keys "please fetch the changelog" Enter
wait_for_text "\(esc\)" approval 300
snap 26-approval
keys Escape
wait_for_text "You denied" denied
wait_idle denied
# The mock's child for this asks for web_fetch itself: a descendant's approval.
keys "delegate the changelog to a child" Enter
wait_for_text "child of assistant" child-approval 300
snap 26a-child-approval
keys Escape
wait_for_text "worker( [0-9]+)? what to do instead" child-denied
# The denial switches to the child before its resumed turn has started.
wait_for_text "Worked for" child-completed 300
# Ctrl+C would interrupt a running turn rather than quit.
wait_idle child-denied
quit_and_capture
