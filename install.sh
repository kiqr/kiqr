#!/bin/sh
# KIQR installer for Linux and macOS — https://kiqr.dev/install.sh
#
#   curl -fsSL https://kiqr.dev/install.sh | sh
#
# Downloads the latest KIQR release for this machine, checks its SHA-256,
# puts the `kiqr` binary on your PATH and registers the daemon to start at
# login. Run it again to update; it says when you already have the latest.
# On Windows, use install.ps1:  irm https://kiqr.dev/install.ps1 | iex
#
#   KIQR_VERSION=v0.3.0      install that release instead of the latest (pre-releases too)
#   KIQR_BIN_DIR=~/bin       where the binary goes (default ~/.local/bin)
#   KIQR_FROM_SOURCE=1       build from source with Cargo instead; KIQR_REF picks the branch or tag
#   KIQR_NO_SERVICE=1        don't register the daemon; start it with `kiqr start`
#   KIQR_HOME=<dir>          where the daemon keeps its data (default ~/.kiqr)
#   KIQR_AUTO_UPDATE=1       install new releases on their own (same as `kiqr update --auto on`)
#   KIQR_RELEASES=<url>      a mirror of github.com/kiqr/kiqr/releases
set -eu

REPO="kiqr/kiqr"
RELEASES="${KIQR_RELEASES:-https://github.com/$REPO/releases}"
VERSION="${KIQR_VERSION:-latest}"
BIN_DIR="${KIQR_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
fail() { printf 'kiqr install: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# ---------------------------------------------------------------- platform

target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) return 1 ;;
  esac
  case "$os" in
    Linux) echo "$arch-unknown-linux-musl" ;;
    Darwin)
      # A shell under Rosetta reports x86_64 on an Apple silicon Mac; install the native build.
      if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then arch=aarch64; fi
      echo "$arch-apple-darwin" ;;
    MINGW* | MSYS* | CYGWIN*) fail "on Windows, run this in PowerShell instead:  irm https://kiqr.dev/install.ps1 | iex" ;;
    *) return 1 ;;
  esac
}

download() { # url file
  if have curl; then
    curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$2" "$1"
  elif have wget; then
    wget -q -O "$2" "$1"
  else
    fail "curl or wget is required."
  fi
}

sha256() {
  if have sha256sum; then sha256sum "$1" | cut -d' ' -f1
  elif have shasum; then shasum -a 256 "$1" | cut -d' ' -f1
  else fail "sha256sum or shasum is required to check the download."
  fi
}

installed_version() {
  [ -x "$BIN_DIR/kiqr" ] && "$BIN_DIR/kiqr" --version 2>/dev/null | awk '{ print $2 }' || true
}

# ---------------------------------------------------------------- install

from_release() {
  t=$1
  if [ "$VERSION" = latest ]; then
    base="$RELEASES/latest/download"
    archive="kiqr-$t.tar.gz"
  else
    v="v${VERSION#v}"
    base="$RELEASES/download/$v"
    archive="kiqr-$v-$t.tar.gz"
  fi

  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT INT TERM
  say "Downloading KIQR ($VERSION) for $t..."
  download "$base/$archive" "$tmp/$archive" || fail "could not download $base/$archive"
  download "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "could not download the checksums"
  want=$(awk -v f="$archive" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
  [ -n "$want" ] || fail "$archive is not in the release's SHA256SUMS"
  got=$(sha256 "$tmp/$archive")
  [ "$want" = "$got" ] || fail "checksum mismatch for $archive (expected $want, got $got)"

  tar -xzf "$tmp/$archive" -C "$tmp"
  bin=$(find "$tmp" -type f -name kiqr | head -n 1)
  [ -n "$bin" ] || fail "the archive holds no kiqr binary"
  new=$("$bin" --version 2>/dev/null | awk '{ print $2 }') || true
  [ -n "$new" ] || fail "the downloaded binary does not run on this machine"

  if [ "$new" = "$before" ]; then
    say "KIQR $new is already the latest."
    return 0
  fi
  mkdir -p "$BIN_DIR"
  # Copy beside it, then rename over it: a running daemon keeps its old copy until it restarts.
  cp "$bin" "$BIN_DIR/.kiqr.new"
  chmod 755 "$BIN_DIR/.kiqr.new"
  mv -f "$BIN_DIR/.kiqr.new" "$BIN_DIR/kiqr"
}

from_source() {
  ref="${KIQR_REF:-main}"
  have git || fail "git is required to build from source."
  have cc || have gcc || have clang || say "note: a C compiler (gcc/clang) is needed to build some dependencies."
  if ! have cargo; then
    say "Rust is not installed. Installing rustup (stable toolchain)..."
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  fi
  case "$ref" in
    v[0-9]*) pick="--tag $ref" ;;
    *) pick="--branch $ref" ;;
  esac
  say "Building KIQR ($ref) from source; this takes a few minutes the first time..."
  root=$(dirname "$BIN_DIR")
  [ "$(basename "$BIN_DIR")" = bin ] || fail "KIQR_BIN_DIR must end in /bin to build from source"
  # shellcheck disable=SC2086
  cargo install --locked --force --git "https://github.com/$REPO" $pick --root "$root" kiqr || fail "the build failed; see the output above."
}

# ---------------------------------------------------------------- daemon

autostart=no
register_daemon() {
  [ "${KIQR_NO_SERVICE:-}" = 1 ] && return 0
  autostart=yes
  if [ "$(uname)" = Linux ] && have systemctl; then
    unit="$HOME/.config/systemd/user/kiqr.service"
    mkdir -p "$(dirname "$unit")"
    cat > "$unit" <<EOF
[Unit]
Description=KIQR daemon
After=network.target

[Service]
ExecStart=$BIN_DIR/kiqr daemon
Restart=on-failure
Environment=PATH=$BIN_DIR:$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin
${KIQR_HOME:+Environment=KIQR_HOME=$KIQR_HOME}

[Install]
WantedBy=default.target
EOF
    if systemctl --user daemon-reload 2>/dev/null; then
      systemctl --user enable kiqr.service >/dev/null 2>&1 || true
      # restart starts it when it is stopped, and picks up the new binary when it runs.
      systemctl --user restart kiqr.service >/dev/null 2>&1 || true
      keep_running_after_logout
    else
      # No user session manager to talk to (su, some containers): run it in the background instead.
      autostart=no
      "$BIN_DIR/kiqr" restart >/dev/null 2>&1 || true
    fi
  elif [ "$(uname)" = Darwin ]; then
    plist="$HOME/Library/LaunchAgents/dev.kiqr.daemon.plist"
    mkdir -p "$(dirname "$plist")"
    cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>dev.kiqr.daemon</string>
  <key>ProgramArguments</key><array><string>$BIN_DIR/kiqr</string><string>daemon</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>EnvironmentVariables</key><dict><key>PATH</key><string>$BIN_DIR:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>${KIQR_HOME:+<key>KIQR_HOME</key><string>$KIQR_HOME</string>}</dict>
</dict></plist>
EOF
    launchctl unload "$plist" >/dev/null 2>&1 || true
    launchctl load -w "$plist" || true
  else
    # No service manager to register with: run it in the background (restarting an older one).
    autostart=no
    "$BIN_DIR/kiqr" restart >/dev/null 2>&1 || true
  fi
}

# A user service stops when its user logs out. On a server (no desktop
# session) that would be the end of the SSH session, so keep it running.
keep_running_after_logout() {
  [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] && return 0
  have loginctl || return 0
  user=$(id -un)
  [ "$(loginctl show-user "$user" -p Linger --value 2>/dev/null)" = yes ] && return 0
  if loginctl enable-linger "$user" 2>/dev/null; then
    say "KIQR keeps running after you log out (loginctl enable-linger $user)."
  else
    say "note: KIQR stops when you log out. To keep it running on a server, run:  sudo loginctl enable-linger $user"
  fi
}

# Wait for the daemon to answer, then say how it runs.
wait_running() {
  [ "${KIQR_NO_SERVICE:-}" = 1 ] && return 0
  i=0
  while [ $i -lt 30 ]; do
    if "$BIN_DIR/kiqr" status >/dev/null 2>&1; then
      "$BIN_DIR/kiqr" status 2>/dev/null | sed -n '1,2p'
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  say "note: the daemon did not answer yet; check it with: kiqr status"
}

# ---------------------------------------------------------------- main

before=$(installed_version)
if [ "${KIQR_FROM_SOURCE:-}" = 1 ]; then
  from_source
elif t=$(target); then
  from_release "$t"
else
  say "There is no prebuilt KIQR for $(uname -s) $(uname -m); building from source instead."
  from_source
fi
after=$(installed_version)
[ -n "$after" ] || fail "kiqr is not in $BIN_DIR after installing."

[ "$after" = "$before" ] && [ "${KIQR_FROM_SOURCE:-}" != 1 ] && exit 0
register_daemon
wait_running
if [ "${KIQR_AUTO_UPDATE:-}" = 1 ]; then "$BIN_DIR/kiqr" update --auto on || true; fi

have git || say "note: KIQR keeps your data in a git repository; install git before you start."
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "Add $BIN_DIR to your PATH, e.g.:  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac

say ""
if [ -n "$before" ]; then
  say "Updated KIQR $before → $after."
else
  if [ "$autostart" = yes ]; then say "KIQR $after is installed and starts at login."; else say "KIQR $after is installed and running. Nothing here starts it at login: run kiqr start after a restart."; fi
  say "Open http://127.0.0.1:4040 or run:"
  say "  kiqr project add https://your-site.com --watch"
  say "KIQR sends an anonymous usage ping to kiqr.cloud (no names, sites or keys); see it or stop it with: kiqr telemetry off"
  if [ -n "${SSH_CONNECTION:-}" ]; then
    say ""
    say "KIQR listens on this server only. To open it from your computer, run this there (it forwards"
    say "the port and opens no shell; leave it running while you use KIQR):"
    say "  ssh -N -L 4040:127.0.0.1:4040 $(id -un)@$(echo "$SSH_CONNECTION" | awk '{ print $3 }')"
    say "then open http://127.0.0.1:4040 in your browser."
    say "To have it install new releases on its own:  kiqr update --auto on"
  fi
fi
say "Manage it with: kiqr status | start | stop | restart | update"
