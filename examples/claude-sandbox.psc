# Launch a sandboxed podman container for Claude Code.
#
# Usage: portascript claude-sandbox.psc [--rebuild] [claude|bash]
#
# Companion files (in script_dir):
#   - claude-sandbox-seccomp.json   custom seccomp profile (allows io_uring)
#   - claude-sandbox-settings.json  container-specific Claude settings (chime hooks)
#   - claude-sandbox-claude.md      sandbox instructions appended to host CLAUDE.md
#   - claude-chime-notify.sh        notification chime script
#   - assets/chime.wav              chime sound file

let script_dir = path.parent(path.abs(args[0]))
let home = env.HOME

# --- Colors ---

let red = "\033[0;31m"
let green = "\033[0;32m"
let yellow = "\033[1;33m"
let nc = "\033[0m"

fn info(msg: str) {
    eprintln("{green}[*]{nc} {msg}")
}

fn warn(msg: str) {
    eprintln("{yellow}[!]{nc} {msg}")
}

fn die(msg: str) {
    eprintln("{red}[ERROR]{nc} {msg}")
    exit(1)
}

# --- Argument parsing ---

let mut rebuild = false
let mut command = "claude"
for arg in args[1..] {
    match arg {
        "--rebuild" => rebuild = true
        "claude" | "bash" => command = arg
        _ => die("Usage: portascript claude-sandbox.psc [--rebuild] [claude|bash]")
    }
}

let container_name = "claude-sandbox-{pid()}"
let workdir = $(exec pwd)

# --- Preflight checks ---

if not command_exists("podman") {
    die("podman not found")
}

if workdir == home {
    die("refusing to run from home directory -- cd into a project first")
}

for f in ["claude-sandbox-seccomp.json", "claude-chime-notify.sh", "assets/chime.wav"] {
    if not path.is_file(path.join(script_dir, f)) {
        die("missing companion file: {script_dir}/{f}")
    }
}

if not path.is_dir(path.join(home, ".local/share/claude")) {
    die("Claude Code not installed -- run: curl -fsSL https://claude.ai/install.sh | bash")
}

if not path.is_dir(path.join(home, ".claude")) {
    warn("no ~/.claude directory -- you will need to authenticate inside the sandbox")
}
if not path.is_file(path.join(home, ".gitconfig")) {
    warn("no ~/.gitconfig -- git commits will fail without a user identity")
}

# --- Volume mounts ---
# Built as a flat list of ["-v", "src:dst:opts", ...] pairs for spreading into podman args.

let mut mounts = ["-v", "{workdir}:{workdir}:Z"]

fn mount_if_file(src: str, dst: str, opts: str) {
    if path.is_file(src) {
        if opts != "" {
            mounts = append(mounts, "-v")
            mounts = append(mounts, "{src}:{dst}:{opts}")
        } else {
            mounts = append(mounts, "-v")
            mounts = append(mounts, "{src}:{dst}")
        }
    }
}

fn mount_if_dir(src: str, dst: str, opts: str) {
    if path.is_dir(src) {
        if opts != "" {
            mounts = append(mounts, "-v")
            mounts = append(mounts, "{src}:{dst}:{opts}")
        } else {
            mounts = append(mounts, "-v")
            mounts = append(mounts, "{src}:{dst}")
        }
    }
}

# Git config (no SSH keys)
mount_if_file(path.join(home, ".gitconfig"), "/home/claude/.gitconfig", "ro")
mount_if_file(path.join(home, ".gitignore"), "/home/claude/.gitignore", "ro")

# Claude config/auth (read-write for OAuth tokens)
mount_if_dir(path.join(home, ".claude"), "/home/claude/.claude", "")
mount_if_file(path.join(home, ".claude.json"), "/home/claude/.claude.json", "")

# Claude binary (read-only)
mount_if_dir(path.join(home, ".local/bin"), "/home/claude/.local/bin", "ro")
mount_if_dir(path.join(home, ".local/share/claude"), "/home/claude/.local/share/claude", "ro")
# Also mount at host $HOME path -- Claude Code may resolve via original install path.
mount_if_dir(path.join(home, ".local/share/claude"), path.join(home, ".local/share/claude"), "ro")

# Override settings.json with container-specific paths for hooks
mount_if_file(path.join(script_dir, "claude-sandbox-settings.json"), "/home/claude/.claude/settings.json", "ro")

# Composite CLAUDE.md: host global instructions + sandbox-specific addendum
let sandbox_claude_md = tempfile()
let host_claude_md = path.join(home, ".claude/CLAUDE.md")
let sandbox_addendum = path.join(script_dir, "claude-sandbox-claude.md")
if path.is_file(host_claude_md) {
    append_file(sandbox_claude_md, read(host_claude_md))
}
if path.is_file(sandbox_addendum) {
    append_file(sandbox_claude_md, read(sandbox_addendum))
}
mounts = append(mounts, "-v")
mounts = append(mounts, "{sandbox_claude_md}:/home/claude/.claude/CLAUDE.md:ro")

# PipeWire audio socket for notification chimes
let xdg_runtime = env.XDG_RUNTIME_DIR ?? "/run/user/{$(exec id -u)}"
let pipewire_sock = path.join(xdg_runtime, "pipewire-0")
if path.is_socket(pipewire_sock) {
    mounts = append(mounts, "-v")
    mounts = append(mounts, "{pipewire_sock}:/run/user/1000/pipewire-0")
}

# Wayland display socket for GUI apps
let wayland_display = env.WAYLAND_DISPLAY ?? "wayland-0"
let wayland_sock = path.join(xdg_runtime, wayland_display)
if path.is_socket(wayland_sock) {
    mounts = append(mounts, "-v")
    mounts = append(mounts, "{wayland_sock}:/run/user/1000/{wayland_display}")
}

# Rust toolchain (mask config.toml to avoid host-specific paths)
mount_if_dir(path.join(home, ".rustup"), "/home/claude/.rustup", "")
mount_if_dir(path.join(home, ".cargo"), "/home/claude/.cargo", "")
if path.is_dir(path.join(home, ".cargo")) {
    mounts = append(mounts, "-v")
    mounts = append(mounts, "/dev/null:/home/claude/.cargo/config.toml:ro")
}

# Shared sccache directory
let sccache_dir = path.join(home, ".cache/sccache")
exec mkdir -p {sccache_dir}
mounts = append(mounts, "-v")
mounts = append(mounts, "{sccache_dir}:/home/claude/.cache/sccache")

# Shared file exchange directory
let drop_dir_host = path.join(home, ".local/share/claude-sandbox/shared")
let drop_dir_guest = "/home/claude/shared"
exec mkdir -p {drop_dir_host}
mounts = append(mounts, "-v")
mounts = append(mounts, "{drop_dir_host}:{drop_dir_guest}")

# --- Cloud credentials env file ---

let env_file = path.join(env.XDG_CONFIG_HOME ?? path.join(home, ".config"), "claude-sandbox/env")

fn parse_env_file(filepath: str) -> map {
    let mut result = {}
    if not path.is_file(filepath) {
        return result
    }
    for line in lines(read(filepath)) {
        let line = trim(line)
        if line == "" or starts_with(line, "#") {
            continue
        }
        let parts = split(line, "=")
        let key = parts[0]
        let value = join(parts[1..], "=")
        result[key] = value
    }
    return result
}

let sandbox_env = parse_env_file(env_file)

# --- Environment variables ---

let mut envs = [
    "-e", "TERM={env.TERM ?? "xterm-256color"}",
    "-e", "RUSTUP_HOME=/home/claude/.rustup",
    "-e", "CARGO_HOME=/home/claude/.cargo",
    "-e", "JAVA_HOME=/usr/lib/jvm/default-java",
    "-e", "XDG_RUNTIME_DIR=/run/user/1000",
    "-e", "WAYLAND_DISPLAY={wayland_display}",
    "-e", "RUSTC_WRAPPER=sccache",
    "-e", "SCCACHE_DIR=/home/claude/.cache/sccache",
    "-e", "SCCACHE_CACHE_SIZE=20G",
    "-e", "SANDBOX_DROP_DIR_HOST={drop_dir_host}",
    "-e", "SANDBOX_DROP_DIR_GUEST={drop_dir_guest}"
]

# Pass through all variables from the env file
for key in keys(sandbox_env) {
    envs = append(envs, "-e")
    envs = append(envs, "{key}={sandbox_env[key]}")
}

# Git SSH-to-HTTPS rewriting (only when GH_TOKEN is available)
if has_key(sandbox_env, "GH_TOKEN") {
    envs = append(envs, "-e")
    envs = append(envs, "GIT_CONFIG_COUNT=2")
    envs = append(envs, "-e")
    envs = append(envs, "GIT_CONFIG_KEY_0=url.https://github.com/.insteadOf")
    envs = append(envs, "-e")
    envs = append(envs, "GIT_CONFIG_VALUE_0=git@github.com:")
    envs = append(envs, "-e")
    envs = append(envs, "GIT_CONFIG_KEY_1=credential.https://github.com.helper")
    envs = append(envs, "-e")
    envs = append(envs, '!f() { echo username=x-access-token; echo "password=$GH_TOKEN"; }; f')
}

# --- Status banner ---

fn yn(condition: bool) -> str {
    if condition { return "yes" }
    return "no"
}

info("Sandbox: {workdir}")
info("Git config: {yn(path.is_file(path.join(home, ".gitconfig")))}")
info("Claude config: {yn(path.is_dir(path.join(home, ".claude")))}")
info("Rust toolchain: {yn(path.is_dir(path.join(home, ".rustup")))}")
info("sccache: yes (shared at ~/.cache/sccache, 20G limit)")
info("Shared dir: ~/.local/share/claude-sandbox/shared -> /home/claude/shared")
info("perf: yes (container linux-tools)")
info("GitHub: {yn(has_key(sandbox_env, "GH_TOKEN"))}")
info("AWS: {yn(has_key(sandbox_env, "AWS_ACCESS_KEY_ID"))}")
info("Azure: {yn(has_key(sandbox_env, "AZURE_CLIENT_ID"))}")
info("GPU: {yn(path.exists("/dev/dri"))}")
info("Wayland: {yn(path.is_socket(wayland_sock))}")
info("PipeWire: {yn(path.is_socket(pipewire_sock))}")

# --- Build container image ---

let host_uid = $(exec id -u)
let host_gid = $(exec id -g)
let image_name = "claude-sandbox:uid-{host_uid}"

let dockerfile = '''
    FROM docker.io/library/ubuntu:24.04

    ENV DEBIAN_FRONTEND=noninteractive

    RUN apt-get update && apt-get install -y --no-install-recommends \
        curl git ca-certificates gnupg build-essential clang cmake pkg-config libssl-dev nano emacs-nox \
        default-jdk maven \
        pipewire pipewire-audio-client-libraries \
        libwayland-client0 libwayland-cursor0 libwayland-egl1 libxkbcommon0 \
        mesa-vulkan-drivers libvulkan1 libasound2-dev \
        xvfb imagemagick mesa-utils libgl1-mesa-dri libegl-mesa0 \
        libgl1-mesa-dev xorg-dev libx11-xcb-dev libxkbcommon-dev librtmidi-dev apitrace \
        qt6-base-dev qt6-declarative-dev unzip \
        libelf1 libdw1 libunwind8 libnuma1 libslang2 libperl-dev binutils \
        linux-tools-common linux-tools-generic \
        && rm -rf /var/lib/apt/lists/*

    ENV JAVA_HOME=/usr/lib/jvm/default-java

    RUN ln -sf /usr/lib/linux-tools/*/perf /usr/local/bin/perf

    RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null \
        && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
        && apt-get update && apt-get install -y --no-install-recommends gh \
        && rm -rf /var/lib/apt/lists/*

    RUN curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o /tmp/awscli.zip \
        && unzip -q /tmp/awscli.zip -d /tmp \
        && /tmp/aws/install \
        && rm -rf /tmp/awscli.zip /tmp/aws

    RUN curl -fsSL https://packages.microsoft.com/keys/microsoft.asc \
        | gpg --dearmor -o /usr/share/keyrings/microsoft-archive-keyring.gpg \
        && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/microsoft-archive-keyring.gpg] https://packages.microsoft.com/repos/azure-cli/ noble main" \
        > /etc/apt/sources.list.d/azure-cli.list \
        && apt-get update && apt-get install -y --no-install-recommends azure-cli \
        && rm -rf /var/lib/apt/lists/*

    RUN userdel -r ubuntu 2>/dev/null || true \
        && groupdel ubuntu 2>/dev/null || true \
        && groupadd -g __HOST_GID__ claude 2>/dev/null || true \
        && useradd -m -s /bin/bash -u __HOST_UID__ -g __HOST_GID__ claude

    COPY --chown=claude:claude claude-chime-notify.sh /home/claude/.local/bin/claude-chime-notify
    COPY --chown=claude:claude chime.wav /home/claude/.local/share/sounds/chime.wav

    RUN SCCACHE_VERSION=0.10.0 \
        && curl -fsSL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
        | tar xz --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache"

    USER claude
    WORKDIR /home/claude
    RUN curl -fsSL https://claude.ai/install.sh | bash \
        && git config --global --add safe.directory '*'
    '''

# Substitute dynamic values into the raw Dockerfile template.
let dockerfile = replace(dockerfile, "__HOST_UID__", host_uid)
let dockerfile = replace(dockerfile, "__HOST_GID__", host_gid)

# Build context: temp dir with just the assets podman needs for COPY
fn build_image(extra_args: list) {
    let ctx = $(exec mktemp -d)
    exec cp "{script_dir}/claude-chime-notify.sh" {ctx}
    exec cp "{script_dir}/assets/chime.wav" {ctx}
    run echo {dockerfile} |
        exec podman build {extra_args...} -t {image_name} -f - {ctx}
    exec rm -rf {ctx}
}

if rebuild {
    info("Rebuilding sandbox image...")
    build_image(["--no-cache"])
} elif not (try exec podman image exists {image_name}).ok {
    info("Building sandbox image (one-time)...")
    build_image([])
}

# Install chime assets into host ~/.local so they survive the bind-mount
exec mkdir -p "{home}/.local/bin" "{home}/.local/share/sounds"
exec cp "{script_dir}/claude-chime-notify.sh" "{home}/.local/bin/claude-chime-notify"
exec chmod +x "{home}/.local/bin/claude-chime-notify"
exec cp "{script_dir}/assets/chime.wav" "{home}/.local/share/sounds/chime.wav"

info("Starting sandbox...")

# GPU flags are conditional on /dev/dri existing
let mut gpu_flags = []
if path.exists("/dev/dri") {
    gpu_flags = ["--device", "/dev/dri", "--group-add", "video"]
}

# Select the in-container command
let mut run_cmd = "exec bash"
if command == "claude" {
    run_cmd = "claude --dangerously-skip-permissions"
}

let bash_init = 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$JAVA_HOME/bin:$PATH" && ' + run_cmd

let result = try exec podman run -it --rm \
    --name {container_name} \
    --hostname "claude-sandbox" \
    --workdir {workdir} \
    --user claude \
    --userns=keep-id \
    --security-opt label=disable \
    --security-opt "seccomp={script_dir}/claude-sandbox-seccomp.json" \
    --cap-add CAP_PERFMON \
    {gpu_flags...} \
    --cpus=4 \
    --cpu-shares=512 \
    --memory=8g \
    {mounts...} \
    {envs...} \
    {image_name} \
    bash -c {bash_init}

# Attempt to upgrade Claude Code on the host after exiting the sandbox.
info("Checking for Claude Code updates...")
if command_exists("claude") {
    let upgrade = try exec claude update
    if upgrade.ok {
        info("Claude Code updated.")
    } else {
        info("Already up to date (or update unavailable).")
    }
} else {
    warn("claude not found on host PATH; skipping update.")
}

exit(result.code)
