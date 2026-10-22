#!/bin/sh
# KIQR installer — https://kiqr.dev/install.sh
#
#   curl -fsSL https://kiqr.dev/install.sh | sh
#
# Builds KIQR from source with Cargo (installing rustup if needed), puts the
# `kiqr` binary on your PATH, and registers the daemon to start at login.
# Set KIQR_REF to install a branch or tag (default: main).
set -eu

REPO="https://github.com/kiqr/kiqr"
REF="${KIQR_REF:-main}"
BIN_DIR="${KIQR_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() { printf 'kiqr install: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

say "Installing KIQR ($REF)"

have git || fail "git is required. Install it and run this again."
have cc || have gcc || have clang || say "note: a C compiler (gcc/clang) is needed to build some dependencies."

if ! have cargo; then
  say "Rust is not installed. Installing rustup (stable toolchain)..."
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

mkdir -p "$BIN_DIR"
say "Building kiqr (this takes a few minutes the first time)..."
cargo install --locked --git "$REPO" --branch "$REF" --root "${KIQR_CARGO_ROOT:-$HOME/.local}" kiqr 2>&1 | grep -v '^\s*Compiling' || true
[ -x "$BIN_DIR/kiqr" ] || fail "build failed; see the output above."

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "Add $BIN_DIR to your PATH, e.g.:  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

# Start the daemon at login.
if [ "$(uname)" = "Linux" ] && have systemctl; then
  mkdir -p "$HOME/.config/systemd/user"
  cat > "$HOME/.config/systemd/user/kiqr.service" <<EOF
[Unit]
Description=KIQR daemon
After=network.target

[Service]
ExecStart=$BIN_DIR/kiqr daemon
Restart=on-failure
Environment=PATH=$BIN_DIR:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin

[Install]
WantedBy=default.target
EOF
  if systemctl --user daemon-reload 2>/dev/null; then
    systemctl --user enable --now kiqr.service >/dev/null 2>&1 && say "Daemon registered with systemd (user) and started."
  fi
elif [ "$(uname)" = "Darwin" ]; then
  PLIST="$HOME/Library/LaunchAgents/dev.kiqr.daemon.plist"
  mkdir -p "$HOME/Library/LaunchAgents"
  cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>dev.kiqr.daemon</string>
  <key>ProgramArguments</key><array><string>$BIN_DIR/kiqr</string><string>daemon</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>EnvironmentVariables</key><dict><key>PATH</key><string>$BIN_DIR:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin</string></dict>
</dict></plist>
EOF
  launchctl unload "$PLIST" >/dev/null 2>&1 || true
  launchctl load "$PLIST" && say "Daemon registered with launchd and started."
fi

say ""
say "KIQR is installed. Open http://127.0.0.1:7777 or run:"
say "  kiqr project add https://your-site.com --watch"
