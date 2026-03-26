#!/bin/bash

# A script to set up my tmux sessions.
#
# Here are the sessions and windows i want.
# Most windows should just sit at the bash prompt.
#
# - session "log"
#   - window 1: cd ~/brson.github.com
#   - window 2: cd ~/brson.github.com
# - session "http"
#   - window 1: terminal
# - session "assistant"
#   - window 1: cd ~/assistant
#   - window 2: "
#   - window 3: "
# - session "datalove1"
#   - window 1: cd ~/megaspace/datalove
#   - window 2: "
#   - window 3: "
# - session "datalove2"
#   - window 1: cd ~/megaspace/datalove2
#   - window 2: "
#   - window 3: "
# - session "sched1"
#   - window 1: cd ~/schedaddlers
#   - window 2: "
#   - window 3: "
# - session "sched2"
#   - window 1: cd ~/schedaddlers2
#   - window 2: "
#   - window 3: "
# - session "synth41"
#   - window 1: cd ~/megaspace/synth4
#   - window 2: "
#   - window 3: "
# - session "synth42"
#   - window 1: cd ~/megaspace/synth42
#   - window 2: "
#   - window 3: "
# - session "rustmax1"
#   - window 1: cd ~/megaspace/rustmax
#   - window 2: "
#   - window 3: "
# - session "rustmax2"
#   - window 1: cd ~/megaspace/rustmax2
#   - window 2: "
#   - window 3: "
# - session "mediadb1"
#   - window 1: cd ~/megaspace/mediadb
#   - window 2: "
#   - window 3: "
# - session "mediadb2"
#   - window 1: cd ~/megaspace/mediadb2
#   - window 2: "
#   - window 3: "
# - session "stakebot"
#   - window 1: cd ~/stakebot
#   - window 2: "
#   - window 3: "
# - session "optbot"
#   - window 1: cd ~/optbot
#   - window 2: "
#   - window 3: "

set -euo pipefail

# Use the same tmux socket as the shell alias (tmux -L $CONTAINER_ID).
if [[ -n "${CONTAINER_ID:-}" ]]; then
    TMUX_CMD=(tmux -L "$CONTAINER_ID")
else
    TMUX_CMD=(tmux)
fi

# Helper: create a session with N windows, all cd'd to the same directory.
# Skip if the session already exists.
# Usage: make_session <name> <dir> <num_windows>
make_session() {
    local name="$1"
    local dir="$2"
    local num_windows="$3"

    if "${TMUX_CMD[@]}" has-session -t "$name" 2>/dev/null; then
        return
    fi

    "${TMUX_CMD[@]}" new-session -d -s "$name" -c "$dir"
    for ((i = 2; i <= num_windows; i++)); do
        "${TMUX_CMD[@]}" new-window -t "$name" -c "$dir"
    done
    # Select first window.
    "${TMUX_CMD[@]}" select-window -t "$name:1"
}

# Kill existing server to start fresh (optional — comment out to be additive).
# "${TMUX_CMD[@]}" kill-server 2>/dev/null || true

# --- log ---
make_session log "$HOME/brson.github.com" 2

# --- http ---
make_session http "$HOME" 1

# --- scratch ---
make_session http "$HOME" 1

# --- project sessions (dir + N windows) ---
make_session assistant    "$HOME/assistant"            3
make_session devscripts   "$HOME/megaspace/devscripts" 3
make_session megaspace    "$HOME/megaspace-one"        3
make_session synth41      "$HOME/megaspace/synth4"     3
make_session synth42      "$HOME/megaspace/synth42"    3
make_session synth43      "$HOME/megaspace/synth43"    3
make_session synth44      "$HOME/megaspace/synth44"    3
make_session mediadb1     "$HOME/megaspace/mediadb"    3
make_session mediadb2     "$HOME/megaspace/mediadb2"   3
make_session datalove1    "$HOME/megaspace/datalove"   3
make_session datalove2    "$HOME/megaspace/datalove2"  3
make_session sched1       "$HOME/schedaddlers"         3
make_session sched2       "$HOME/schedaddlers2"        3
make_session rustmax1     "$HOME/megaspace/rustmax"    3
make_session rustmax2     "$HOME/megaspace/rustmax2"   3
make_session tokenbub     "$HOME/tokenbub"             3
make_session musicsite    "$HOME/musicsite"            3
make_session bamusic      "$HOME/bamusic"              3
make_session stakebot     "$HOME/stakebot"             3
make_session optbot       "$HOME/optbot"               3
make_session polywhatever "$HOME/polywhatever"         3

# Done. Attach manually with: tmux attach -t log
