#!/usr/bin/env bash
# RS-Ops installer for Linux.
# Honest scope: builds from source with cargo (no prebuilt binary signing/
# packaging pipeline exists yet), installs to ~/.local/rs-ops, and
# offers to set up a systemd --user service. Requires: git, curl, a Rust
# toolchain (installed automatically via rustup if missing).
set -euo pipefail

INSTALL_DIR="${RS_OPS_HOME:-$HOME/.local/rs-ops}"
REPO_URL="https://github.com/aon-co-jp/RS-Ops.git"

echo "== RS-Ops installer (Linux) =="

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
echo "Build complete: $INSTALL_DIR/target/release/rs-ops"
echo "Run manually with:"
echo "  RS_OPS_PORT=8097 RS_OPS_SCAN_ROOT=/path/to/your/repos $INSTALL_DIR/target/release/rs-ops"
echo
read -r -p "Install as a systemd --user service (auto-start on login)? [y/N] " ans
if [[ "$ans" =~ ^[Yy]$ ]]; then
  read -r -p "Path to scan for projects [default: \$HOME]: " scan_root
  scan_root="${scan_root:-$HOME}"
  mkdir -p "$HOME/.config/systemd/user"
  cat > "$HOME/.config/systemd/user/rs-ops.service" <<EOF
[Unit]
Description=RS-Ops ecosystem map + AI-agent context generator

[Service]
Environment=RS_OPS_PORT=8097
Environment=RS_OPS_SCAN_ROOT=$scan_root
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/target/release/rs-ops
Restart=on-failure

[Install]
WantedBy=default.target
EOF
  systemctl --user daemon-reload
  systemctl --user enable --now rs-ops.service
  echo "Started rs-ops.service (systemctl --user status rs-ops)"
fi

echo "Done. Open http://127.0.0.1:8097/ once running."
