# Set up tmux sessions for development.
#
# Each session gets N windows, all cd'd to the same directory.
# Sessions that already exist are skipped.

# Use the same tmux socket as the shell alias (tmux -L $CONTAINER_ID).
let socket = env.CONTAINER_ID ?? ""
let mut tmux_prefix = []
if socket != "" {
    tmux_prefix = ["-L", socket]
}

# Create a session with N windows, all cd'd to dir.
# Skip if the session already exists.
fn make_session(name: str, dir: str, num_windows: int) {
    let r = try exec tmux {tmux_prefix...} has-session -t {name}
    if r.ok {
        return
    }

    exec tmux {tmux_prefix...} new-session -d -s {name} -c {dir}
    for i in range(2, num_windows + 1) {
        exec tmux {tmux_prefix...} new-window -t {name} -c {dir}
    }
    exec tmux {tmux_prefix...} select-window -t "{name}:1"
}

let home = env.HOME

# --- log ---
make_session("log", path.join(home, "brson.github.com"), 2)

# --- http ---
make_session("http", home, 1)

# --- scratch ---
make_session("scratch", home, 1)

# --- project sessions ---
make_session("assistant",    path.join(home, "assistant"),            3)
make_session("devscripts",   path.join(home, "megaspace/devscripts"), 3)
make_session("megaspace",    path.join(home, "megaspace-one"),        3)
make_session("synth41",      path.join(home, "megaspace/synth4"),     3)
make_session("synth42",      path.join(home, "megaspace/synth42"),    3)
make_session("synth43",      path.join(home, "megaspace/synth43"),    3)
make_session("synth44",      path.join(home, "megaspace/synth44"),    3)
make_session("mediadb1",     path.join(home, "megaspace/mediadb"),    3)
make_session("mediadb2",     path.join(home, "megaspace/mediadb2"),   3)
make_session("datalove1",    path.join(home, "megaspace/datalove"),   3)
make_session("datalove2",    path.join(home, "megaspace/datalove2"),  3)
make_session("sched1",       path.join(home, "schedaddlers"),         3)
make_session("sched2",       path.join(home, "schedaddlers2"),        3)
make_session("rustmax1",     path.join(home, "megaspace/rustmax"),    3)
make_session("rustmax2",     path.join(home, "megaspace/rustmax2"),   3)
make_session("tokenbub",     path.join(home, "tokenbub"),             3)
make_session("musicsite",    path.join(home, "musicsite"),            3)
make_session("bamusic",      path.join(home, "bamusic"),              3)
make_session("stakebot",     path.join(home, "stakebot"),             3)
make_session("optbot",       path.join(home, "optbot"),               3)
make_session("polywhatever", path.join(home, "polywhatever"),         3)

# Done. Attach manually with: tmux attach -t log
