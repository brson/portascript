#!/usr/bin/env bash
#
# Launch a sandboxed podman container for Claude Code with --dangerously-skip-permissions.
#
# Usage: claude-sandbox.sh [--rebuild] [claude|bash]
#
# Companion files (in SCRIPT_DIR):
#   - claude-sandbox-seccomp.json   custom seccomp profile (allows io_uring)
#   - claude-sandbox-settings.json  container-specific Claude settings (chime hooks)
#   - claude-sandbox-claude.md      sandbox instructions appended to host CLAUDE.md
#   - claude-chime-notify.sh        notification chime script
#   - assets/chime.wav              chime sound file
#
# Container image (Ubuntu 24.04):
#   - Build: gcc, clang, cmake, pkg-config, libssl-dev
#   - Java: default-jdk, maven
#   - Graphics: xvfb, mesa, Vulkan, Qt6, X11/Wayland libs, apitrace
#   - Audio: PipeWire client libraries
#   - Editors: nano, emacs-nox
#   - Profiling: perf (linux-tools, symlinked past kernel version check)
#   - Cloud: aws CLI v2, az CLI, gh CLI
#   - Tools: imagemagick, unzip, sccache
#   - Claude Code (native binary via install.sh)
#   - Notification chime (pw-play / paplay / terminal bell fallback)
#
# Security:
#   - Runs as non-root user "claude" with host UID/GID (--userns=keep-id)
#   - Mounts only the current directory (at host path, for unique project identity)
#   - No SSH keys (intentionally excluded)
#   - Git SSH URLs rewritten to HTTPS (read-only via token, see below)
#   - Custom seccomp profile, SELinux labels disabled
#   - CAP_PERFMON for perf profiling (requires host kernel.perf_event_paranoid <= 1)
#
# Resource limits:
#   - 4 CPUs, cpu-shares=512, 8 GB memory
#
# Passthrough (from host, when available):
#   - Git config (~/.gitconfig, ~/.gitignore) - read-only
#   - Claude config/auth (~/.claude, ~/.claude.json) - read-write
#   - Claude settings overridden with claude-sandbox-settings.json (hooks for chime)
#   - CLAUDE.md composed from host ~/.claude/CLAUDE.md + sandbox addendum (read-only)
#   - Claude binary (~/.local/bin, ~/.local/share/claude) - read-only
#   - Rust toolchain (~/.rustup, ~/.cargo) - cargo config.toml masked with /dev/null
#   - sccache compilation cache (~/.cache/sccache) - shared across all instances
#   - Shared file exchange (~/.local/share/claude-sandbox/shared -> /home/claude/shared)
#   - GPU access (/dev/dri, video group)
#   - Wayland display socket
#   - PipeWire audio socket
#   - Cloud tokens from env file (GH_TOKEN, AWS_*, AZURE_*)
#   - SANDBOX_DROP_DIR_HOST: host-side path to shared file exchange directory
#   - SANDBOX_DROP_DIR_GUEST: container-side path to shared file exchange directory
#
# Cloud credentials:
#   All sandbox-specific credentials are stored in a single env file:
#     ~/.config/claude-sandbox/env (mode 600, KEY=VALUE format)
#   If the file doesn't exist, cloud tokens are silently skipped.
#   Supported variables:
#     GH_TOKEN              GitHub PAT (used for gh CLI and git HTTPS auth)
#     AWS_ACCESS_KEY_ID     AWS credentials (use a scoped IAM user or
#     AWS_SECRET_ACCESS_KEY   STS temporary credentials for least privilege)
#     AWS_SESSION_TOKEN     (optional, for STS/assumed-role temporary creds)
#     AWS_DEFAULT_REGION    (optional, e.g. us-east-1)
#     AZURE_TENANT_ID       Azure service principal credentials (register an
#     AZURE_CLIENT_ID         app in Entra ID, create a client secret, assign
#     AZURE_CLIENT_SECRET     RBAC roles to control sandbox permissions)
#     AZURE_SUBSCRIPTION_ID (optional, default subscription for az CLI)
#   To enforce read-only git access, use a fine-grained GitHub PAT with
#   only "Contents: Read-only" and "Metadata: Read-only" permissions.
#
# Git SSH-to-HTTPS rewriting:
#   The container has no SSH keys, so git-over-SSH would fail. To allow
#   fetching from SSH-style remotes (git@github.com:owner/repo.git), we
#   use git's GIT_CONFIG_COUNT/KEY/VALUE env vars to inject two settings
#   at runtime without modifying the read-only ~/.gitconfig mount:
#
#     url.https://github.com/.insteadOf = git@github.com:
#       Tells git to silently rewrite any "git@github.com:" URL prefix
#       to "https://github.com/" before connecting. This converts SSH
#       remotes into HTTPS remotes, which don't need SSH keys.
#
#     credential.https://github.com.helper = <inline shell function>
#       Provides a git credential helper that returns GH_TOKEN as the
#       password for any HTTPS request to github.com. Git invokes this
#       helper when it needs auth for an HTTPS remote. The helper is a
#       shell one-liner that prints the credential protocol fields:
#         username=x-access-token  (GitHub accepts any username for PATs)
#         password=$GH_TOKEN       (the actual token, from the env var)
#
#   The net effect: `git fetch origin` on a repo with an SSH remote
#   transparently fetches over HTTPS using the GitHub token. Push access
#   depends entirely on the token's scopes -- a read-only PAT means
#   fetches succeed but pushes are rejected by GitHub's API.
#
# Prerequisites:
#   Required:
#     - podman
#     - Claude Code installed on host (~/.local/bin/claude, ~/.local/share/claude)
#     - Companion files in SCRIPT_DIR (listed above)
#   Recommended:
#     - ~/.claude directory (Claude auth/config; without it you must authenticate inside)
#     - ~/.gitconfig (git user identity; without it git commits will fail)
#   Optional:
#     - ~/.config/claude-sandbox/env (cloud credentials, mode 600)
#     - ~/.rustup, ~/.cargo (Rust toolchain)
#     - /dev/dri (GPU access for graphics workloads)
#     - PipeWire socket (notification chimes)
#     - Wayland socket (GUI apps)
#     - kernel.perf_event_paranoid <= 1 (for perf profiling)
#
# Post-exit:
#   Attempts to upgrade Claude Code on the host (in-sandbox install is read-only).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

REBUILD=false
COMMAND="claude"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --rebuild) REBUILD=true; shift ;;
        claude|bash) COMMAND="$1"; shift ;;
        *) echo "Usage: $0 [--rebuild] [claude|bash]" >&2; exit 1 ;;
    esac
done

CONTAINER_NAME="claude-sandbox-$$"
# Use the host path as the container mount point so Claude Code derives a
# unique project identity per directory (instead of everything being "/workspace").
WORKDIR="$(pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[*]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

command -v podman >/dev/null || die "podman not found"

# Mounting $HOME as the workspace conflicts with individual home directory mounts
# (.claude, .cargo, .rustup, etc.) and causes podman to hang.
[[ "$(pwd)" == "$HOME" ]] && die "refusing to run from home directory -- cd into a project first"

# Companion files
for f in claude-sandbox-seccomp.json claude-chime-notify.sh assets/chime.wav; do
    [[ -f "$SCRIPT_DIR/$f" ]] || die "missing companion file: $SCRIPT_DIR/$f"
done

# Claude Code binary
if [[ ! -d "$HOME/.local/share/claude" ]]; then
    die "Claude Code not installed -- run: curl -fsSL https://claude.ai/install.sh | bash"
fi

# Warnings for recommended config
[[ -d "$HOME/.claude" ]] || warn "no ~/.claude directory -- you will need to authenticate inside the sandbox"
[[ -f "$HOME/.gitconfig" ]] || warn "no ~/.gitconfig -- git commits will fail without a user identity"

# Volume mounts
mounts=(
    "-v" "$WORKDIR:$WORKDIR:Z"
)

# Git config (no SSH keys)
[[ -f "$HOME/.gitconfig" ]] && mounts+=("-v" "$HOME/.gitconfig:/home/claude/.gitconfig:ro")
[[ -f "$HOME/.gitignore" ]] && mounts+=("-v" "$HOME/.gitignore:/home/claude/.gitignore:ro")

# Claude config/auth (read-write for OAuth tokens)
[[ -d "$HOME/.claude" ]] && mounts+=("-v" "$HOME/.claude:/home/claude/.claude")
[[ -f "$HOME/.claude.json" ]] && mounts+=("-v" "$HOME/.claude.json:/home/claude/.claude.json")
# Claude binary (read-only to prevent in-sandbox upgrades from breaking host symlink)
[[ -d "$HOME/.local/bin" ]] && mounts+=("-v" "$HOME/.local/bin:/home/claude/.local/bin:ro")
[[ -d "$HOME/.local/share/claude" ]] && mounts+=("-v" "$HOME/.local/share/claude:/home/claude/.local/share/claude:ro")
# Also mount at the host $HOME path — Claude Code may resolve its binary via the original install path.
[[ -d "$HOME/.local/share/claude" ]] && mounts+=("-v" "$HOME/.local/share/claude:$HOME/.local/share/claude:ro")
# Override settings.json with container-specific paths for hooks
[[ -f "$SCRIPT_DIR/claude-sandbox-settings.json" ]] && mounts+=("-v" "$SCRIPT_DIR/claude-sandbox-settings.json:/home/claude/.claude/settings.json:ro")
# Composite CLAUDE.md: host global instructions + sandbox-specific addendum
SANDBOX_CLAUDE_MD=$(mktemp)
trap 'rm -f "$SANDBOX_CLAUDE_MD"' EXIT
[[ -f "$HOME/.claude/CLAUDE.md" ]] && cat "$HOME/.claude/CLAUDE.md" >> "$SANDBOX_CLAUDE_MD"
[[ -f "$SCRIPT_DIR/claude-sandbox-claude.md" ]] && cat "$SCRIPT_DIR/claude-sandbox-claude.md" >> "$SANDBOX_CLAUDE_MD"
mounts+=("-v" "$SANDBOX_CLAUDE_MD:/home/claude/.claude/CLAUDE.md:ro")

# PipeWire audio socket for notification chimes
XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
[[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]] && mounts+=("-v" "$XDG_RUNTIME_DIR/pipewire-0:/run/user/1000/pipewire-0")

# Wayland display socket for GUI apps
WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
[[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && mounts+=("-v" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/run/user/1000/$WAYLAND_DISPLAY")

# Rust toolchain (mask config.toml to avoid host-specific paths)
[[ -d "$HOME/.rustup" ]] && mounts+=("-v" "$HOME/.rustup:/home/claude/.rustup")
[[ -d "$HOME/.cargo" ]] && mounts+=("-v" "$HOME/.cargo:/home/claude/.cargo")
[[ -d "$HOME/.cargo" ]] && mounts+=("-v" "/dev/null:/home/claude/.cargo/config.toml:ro")


# perf is installed in the container image via linux-tools-common/generic.
# Kernel version mismatch is expected (container perf != host kernel) but
# basic functionality (stat, record, report) generally works regardless.

# Shared sccache directory (compilation cache shared across all sandbox instances)
mkdir -p "$HOME/.cache/sccache"
mounts+=("-v" "$HOME/.cache/sccache:/home/claude/.cache/sccache")

# Shared file exchange directory (accessible from both host and all sandbox instances)
SANDBOX_DROP_DIR_HOST="$HOME/.local/share/claude-sandbox/shared"
SANDBOX_DROP_DIR_GUEST="/home/claude/shared"
mkdir -p "$SANDBOX_DROP_DIR_HOST"
mounts+=("-v" "$SANDBOX_DROP_DIR_HOST:$SANDBOX_DROP_DIR_GUEST")

# Cloud credentials env file
SANDBOX_ENV_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/claude-sandbox/env"

# Parse env file into an associative array (KEY=VALUE, # comments, blank lines skipped)
declare -A sandbox_env
if [[ -f "$SANDBOX_ENV_FILE" ]]; then
    while IFS= read -r line; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        key="${line%%=*}"
        value="${line#*=}"
        sandbox_env["$key"]="$value"
    done < "$SANDBOX_ENV_FILE"
fi

# Environment variables
envs=(
    "-e" "TERM=${TERM:-xterm-256color}"
    "-e" "RUSTUP_HOME=/home/claude/.rustup"
    "-e" "CARGO_HOME=/home/claude/.cargo"
    "-e" "JAVA_HOME=/usr/lib/jvm/default-java"
    "-e" "XDG_RUNTIME_DIR=/run/user/1000"
    "-e" "WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-wayland-0}"
    "-e" "RUSTC_WRAPPER=sccache"
    "-e" "SCCACHE_DIR=/home/claude/.cache/sccache"
    "-e" "SCCACHE_CACHE_SIZE=20G"
    "-e" "SANDBOX_DROP_DIR_HOST=$SANDBOX_DROP_DIR_HOST"
    "-e" "SANDBOX_DROP_DIR_GUEST=$SANDBOX_DROP_DIR_GUEST"
)

# Pass through all variables from the env file
for key in "${!sandbox_env[@]}"; do
    envs+=("-e" "$key=${sandbox_env[$key]}")
done

# Git SSH-to-HTTPS rewriting (only when GH_TOKEN is available).
# See "Git SSH-to-HTTPS rewriting" in the header for full explanation.
if [[ -n "${sandbox_env[GH_TOKEN]:-}" ]]; then
    envs+=(
        "-e" "GIT_CONFIG_COUNT=2"
        "-e" "GIT_CONFIG_KEY_0=url.https://github.com/.insteadOf"
        "-e" "GIT_CONFIG_VALUE_0=git@github.com:"
        "-e" "GIT_CONFIG_KEY_1=credential.https://github.com.helper"
        "-e" 'GIT_CONFIG_VALUE_1=!f() { echo username=x-access-token; echo "password=$GH_TOKEN"; }; f'
    )
fi
info "Sandbox: $(pwd) -> ${WORKDIR}"
info "Git config: $([[ -f "$HOME/.gitconfig" ]] && echo "yes" || echo "no")"
info "Claude config: $([[ -d "$HOME/.claude" ]] && echo "yes" || echo "no")"
info "Rust toolchain: $([[ -d "$HOME/.rustup" ]] && echo "yes" || echo "no")"
info "sccache: yes (shared at ~/.cache/sccache, 20G limit)"
info "Shared dir: ~/.local/share/claude-sandbox/shared -> /home/claude/shared"
info "perf: yes (container linux-tools)"
info "GitHub: $([[ -n "${sandbox_env[GH_TOKEN]:-}" ]] && echo "yes" || echo "no (set GH_TOKEN in $SANDBOX_ENV_FILE)")"
info "AWS: $([[ -n "${sandbox_env[AWS_ACCESS_KEY_ID]:-}" ]] && echo "yes" || echo "no")"
info "Azure: $([[ -n "${sandbox_env[AZURE_CLIENT_ID]:-}" ]] && echo "yes" || echo "no")"
info "GPU: $([[ -e /dev/dri ]] && echo "yes" || echo "no")"
info "Wayland: $([[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && echo "yes" || echo "no")"
info "PipeWire: $([[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]] && echo "yes" || echo "no")"

# Dockerfile for the sandbox image
HOST_UID=$(id -u)
HOST_GID=$(id -g)

dockerfile=$(cat <<DOCKERFILE
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

# Bypass linux-tools kernel version check — symlink the actual perf binary
# so it's found before the wrapper script at /usr/bin/perf
RUN ln -sf /usr/lib/linux-tools/*/perf /usr/local/bin/perf

# Install gh CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null \
    && echo "deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# Install AWS CLI v2
RUN curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o /tmp/awscli.zip \
    && unzip -q /tmp/awscli.zip -d /tmp \
    && /tmp/aws/install \
    && rm -rf /tmp/awscli.zip /tmp/aws

# Install Azure CLI
RUN curl -fsSL https://packages.microsoft.com/keys/microsoft.asc \
    | gpg --dearmor -o /usr/share/keyrings/microsoft-archive-keyring.gpg \
    && echo "deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/microsoft-archive-keyring.gpg] https://packages.microsoft.com/repos/azure-cli/ noble main" \
    > /etc/apt/sources.list.d/azure-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends azure-cli \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user with matching UID/GID for --userns=keep-id
# Ubuntu 24.04 has ubuntu:1000 by default, so delete it first if it exists
RUN userdel -r ubuntu 2>/dev/null || true \
    && groupdel ubuntu 2>/dev/null || true \
    && groupadd -g ${HOST_GID} claude 2>/dev/null || true \
    && useradd -m -s /bin/bash -u ${HOST_UID} -g ${HOST_GID} claude

# Install notification chime assets
COPY --chown=claude:claude claude-chime-notify.sh /home/claude/.local/bin/claude-chime-notify
COPY --chown=claude:claude chime.wav /home/claude/.local/share/sounds/chime.wav

# Install sccache (shared compilation cache across sandbox instances)
RUN SCCACHE_VERSION=0.10.0 \
    && curl -fsSL "https://github.com/mozilla/sccache/releases/download/v\${SCCACHE_VERSION}/sccache-v\${SCCACHE_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz --strip-components=1 -C /usr/local/bin "sccache-v\${SCCACHE_VERSION}-x86_64-unknown-linux-musl/sccache"

# Install Claude Code native binary
USER claude
WORKDIR /home/claude
RUN curl -fsSL https://claude.ai/install.sh | bash \
    && git config --global --add safe.directory '*'
DOCKERFILE
)

# Build the image if needed (tagged with UID since it's baked in)
IMAGE_NAME="claude-sandbox:uid-${HOST_UID}"
# Use minimal build context with just the assets needed
build_image() {
    local ctx
    ctx=$(mktemp -d)
    cp "$SCRIPT_DIR/claude-chime-notify.sh" "$ctx/"
    cp "$SCRIPT_DIR/assets/chime.wav" "$ctx/"
    echo "$dockerfile" | podman build "$@" -t "$IMAGE_NAME" -f - "$ctx"
    rm -rf "$ctx"
}
if $REBUILD; then
    info "Rebuilding sandbox image..."
    build_image --no-cache
elif ! podman image exists "$IMAGE_NAME" 2>/dev/null; then
    info "Building sandbox image (one-time)..."
    build_image
fi

# Install chime assets into host ~/.local so they survive the bind-mount
# (the ~/.local/bin mount overrides what the Dockerfile COPYs into the image)
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/sounds"
cp "$SCRIPT_DIR/claude-chime-notify.sh" "$HOME/.local/bin/claude-chime-notify"
chmod +x "$HOME/.local/bin/claude-chime-notify"
cp "$SCRIPT_DIR/assets/chime.wav" "$HOME/.local/share/sounds/chime.wav"

info "Starting sandbox..."

# GPU flags are conditional on /dev/dri existing
gpu_flags=()
if [[ -e /dev/dri ]]; then
    gpu_flags+=("--device" "/dev/dri" "--group-add" "video")
fi

podman run -it --rm \
    --name "$CONTAINER_NAME" \
    --hostname "claude-sandbox" \
    --workdir "$WORKDIR" \
    --user claude \
    --userns=keep-id \
    --security-opt label=disable \
    --security-opt "seccomp=$SCRIPT_DIR/claude-sandbox-seccomp.json" \
    --cap-add CAP_PERFMON \
    "${gpu_flags[@]}" \
    --cpus=4 \
    --cpu-shares=512 \
    --memory=8g \
    "${mounts[@]}" \
    "${envs[@]}" \
    "$IMAGE_NAME" \
    bash -c 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$JAVA_HOME/bin:$PATH" && '"$(
        case "$COMMAND" in
            claude) echo 'claude --dangerously-skip-permissions' ;;
            bash) echo 'exec bash' ;;
        esac
    )"
container_exit=$?

# Attempt to upgrade Claude Code on the host after exiting the sandbox.
# The in-sandbox install is read-only, so upgrades must happen here.
info "Checking for Claude Code updates..."
if command -v claude >/dev/null 2>&1; then
    claude update 2>/dev/null && info "Claude Code updated." || info "Already up to date (or update unavailable)."
else
    warn "claude not found on host PATH; skipping update."
fi

exit $container_exit
