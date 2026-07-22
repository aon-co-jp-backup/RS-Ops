#!/usr/bin/env bash
# RS-AI-DevOps installer for Linux.
# Honest scope: builds from source with cargo (no prebuilt binary signing/
# packaging pipeline exists yet), installs to ~/.local/rs-ai-devops, and
# offers to set up a systemd --user service. Requires: git, curl, a Rust
# toolchain (installed automatically via rustup if missing).
set -euo pipefail

INSTALL_DIR="${RS_AI_DEVOPS_HOME:-$HOME/.local/rs-ai-devops}"
REPO_URL="https://github.com/aon-co-jp/RS-AI-DevOps.git"

echo "== RS-AI-DevOps installer (Linux) =="

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust toolchain not found; installing via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if [ ! -d "$INSTALL_DIR" ]; then
  git clone "$REPO_URL" "$INSTALL_DIR"
else
  echo "Existing install found at $INSTALL_DIR, pulling latest..."
  git -C "$INSTALL_DIR" pull --ff-only
fi

cd "$INSTALL_DIR"
cargo build --release

echo
echo "Build complete: $INSTALL_DIR/target/release/rs-ai-devops"
echo "Run manually with:"
echo "  RS_AI_DEVOPS_PORT=8097 RS_AI_DEVOPS_SCAN_ROOT=/path/to/your/repos $INSTALL_DIR/target/release/rs-ai-devops"
echo
read -r -p "Install as a systemd --user service (auto-start on login)? [y/N] " ans
if [[ "$ans" =~ ^[Yy]$ ]]; then
  read -r -p "Path to scan for projects [default: \$HOME]: " scan_root
  scan_root="${scan_root:-$HOME}"
  mkdir -p "$HOME/.config/systemd/user"
  cat > "$HOME/.config/systemd/user/rs-ai-devops.service" <<EOF
[Unit]
Description=RS-AI-DevOps ecosystem map + AI-agent context generator

[Service]
Environment=RS_AI_DEVOPS_PORT=8097
Environment=RS_AI_DEVOPS_SCAN_ROOT=$scan_root
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/target/release/rs-ai-devops
Restart=on-failure

[Install]
WantedBy=default.target
EOF
  systemctl --user daemon-reload
  systemctl --user enable --now rs-ai-devops.service
  echo "Started rs-ai-devops.service (systemctl --user status rs-ai-devops)"
fi

echo "Done. Open http://127.0.0.1:8097/ once running."
