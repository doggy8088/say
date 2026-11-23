#!/bin/sh
# Install say, a cross-platform text-to-speech CLI, without Node.js.
#
#   curl -fsSL https://say.gh.miniasp.com/install.sh | sh
#
# Options, as environment variables or as flags after `sh -s --`:
#
#   SAY_INSTALL_DIR   --dir DIR       Folder for the binary. Default: $HOME/.local/bin
#   SAY_VERSION       --version TAG   Release to install, such as v0.1.0. Default: latest
#
#   curl -fsSL https://say.gh.miniasp.com/install.sh | sh -s -- --version v0.1.0
#
# Supports macOS (Apple silicon and Intel) and Linux with glibc (x64 and ARM64).
# On Windows, use install.ps1 instead. Source: https://github.com/doggy8088/say
#
# Everything runs from main() on the last line, so a download cut off halfway
# through runs nothing.

set -eu

REPO="doggy8088/say"
SITE="https://say.gh.miniasp.com"

if [ -t 2 ] && [ -z "${NO_COLOR-}" ]; then
  BOLD=$(printf '\033[1m')
  RED=$(printf '\033[31m')
  YELLOW=$(printf '\033[33m')
  RESET=$(printf '\033[0m')
else
  BOLD=""
  RED=""
  YELLOW=""
  RESET=""
fi

info() { printf '%s\n' "$*"; }
warn() { printf '%swarning:%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die() {
  printf '%serror:%s %s\n' "$RED" "$RESET" "$*" >&2
  exit 1
}
has() { command -v "$1" >/dev/null 2>&1; }

usage() {
  cat <<EOF
Install say from GitHub Releases.

Usage: install.sh [--dir DIR] [--version TAG]

  --dir DIR       Folder for the binary (default: \$HOME/.local/bin, or \$SAY_INSTALL_DIR)
  --version TAG   Release to install, such as v0.1.0 (default: latest, or \$SAY_VERSION)
  -h, --help      Show this help
EOF
}

is_musl() {
  if [ -f /etc/alpine-release ]; then
    return 0
  fi
  if has ldd && ldd --version 2>&1 | grep -qi musl; then
    return 0
  fi
  return 1
}

# Sets TARGET to the Rust target triple of a prebuilt release.
detect_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Darwin) os="apple-darwin" ;;
    Linux) os="unknown-linux-gnu" ;;
    MINGW* | MSYS* | CYGWIN* | Windows_NT)
      die "this script is for macOS and Linux. On Windows, run this in PowerShell: irm $SITE/install.ps1 | iex"
      ;;
    *) die "there is no prebuilt say for $os. Build it from source: cargo install --git https://github.com/$REPO" ;;
  esac

  case "$arch" in
    x86_64 | amd64) arch="x86_64" ;;
    arm64 | aarch64) arch="aarch64" ;;
    *) die "there is no prebuilt say for $arch CPUs. Build it from source: cargo install --git https://github.com/$REPO" ;;
  esac

  # A shell running under Rosetta 2 reports x86_64 on Apple silicon. Use the native build.
  if [ "$os" = "apple-darwin" ] && [ "$arch" = "x86_64" ] &&
    [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
    arch="aarch64"
  fi

  if [ "$os" = "unknown-linux-gnu" ] && is_musl; then
    die "the prebuilt Linux build needs glibc, and this system uses musl (Alpine, for example). Build it from source: cargo install --git https://github.com/$REPO"
  fi

  TARGET="$arch-$os"
}

check_tools() {
  has curl || has wget || die "install curl or wget first, then run this again"
  has sha256sum || has shasum || has openssl ||
    die "install sha256sum, shasum, or openssl first, so the download can be verified"
  has tar || die "install tar first, then run this again"
  # GNU tar hands .xz archives to the xz program; macOS tar reads them itself.
  if [ "$(uname -s)" = "Linux" ] && ! has xz; then
    die "install xz first (Debian/Ubuntu: sudo apt-get install xz-utils; Fedora: sudo dnf install xz), then run this again"
  fi
}

download() {
  if has curl; then
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location --retry 3 --output "$2" "$1"
  else
    wget -q -O "$2" "$1"
  fi
}

fetch_text() {
  if has curl; then
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location --retry 3 "$1"
  else
    wget -q -O - "$1"
  fi
}

sha256_of() {
  if has sha256sum; then
    sha256sum "$1" | awk '{ print $1 }'
  elif has shasum; then
    shasum -a 256 "$1" | awk '{ print $1 }'
  else
    openssl dgst -sha256 "$1" | awk '{ print $NF }'
  fi
}

cleanup() {
  if [ -n "${WORK_DIR-}" ] && [ -d "$WORK_DIR" ]; then
    rm -rf "$WORK_DIR"
  fi
  if [ -n "${STAGED-}" ] && [ -f "$STAGED" ]; then
    rm -f "$STAGED"
  fi
}

# Explains how to put DIR on PATH, or warns when another say comes first.
path_advice() {
  dir=$1
  case ":${PATH-}:" in
    *":$dir:"* | *":$dir/:"*)
      found=$(command -v say 2>/dev/null || true)
      if [ -n "$found" ] && [ "$found" != "$dir/say" ]; then
        warn "running say starts $found, which comes before $dir on your PATH."
        if [ "$found" = "/usr/bin/say" ] && [ "$(uname -s)" = "Darwin" ]; then
          warn "That is the say built into macOS. To use this one instead, move $dir ahead of /usr/bin in PATH."
        fi
        warn "You can always run this one directly: $dir/say"
      fi
      return 0
      ;;
  esac

  case "$dir" in
    "$HOME"/*) shown="\$HOME/${dir#"$HOME"/}" ;;
    *) shown=$dir ;;
  esac

  info ""
  info "$dir is not on your PATH yet. To add it:"
  case "${SHELL##*/}" in
    fish)
      info "  fish_add_path \"$shown\""
      ;;
    zsh)
      info "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.zshrc"
      ;;
    bash)
      if [ "$(uname -s)" = "Darwin" ]; then
        info "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.bash_profile"
      else
        info "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.bashrc"
      fi
      ;;
    *)
      info "  echo 'export PATH=\"$shown:\$PATH\"' >> ~/.profile"
      ;;
  esac
  info "Then open a new terminal."
}

main() {
  install_dir=${SAY_INSTALL_DIR-}
  version=${SAY_VERSION-}

  while [ $# -gt 0 ]; do
    case "$1" in
      -d | --dir)
        [ $# -ge 2 ] || die "$1 needs a folder"
        install_dir=$2
        shift 2
        ;;
      --dir=*)
        install_dir=${1#*=}
        shift
        ;;
      -v | --version)
        [ $# -ge 2 ] || die "$1 needs a release tag, such as v0.1.0"
        version=$2
        shift 2
        ;;
      --version=*)
        version=${1#*=}
        shift
        ;;
      -h | --help)
        usage
        exit 0
        ;;
      *) die "unknown option: $1 (see --help)" ;;
    esac
  done

  if [ -z "$install_dir" ]; then
    [ -n "${HOME-}" ] || die "HOME is not set. Choose a folder with SAY_INSTALL_DIR."
    install_dir="$HOME/.local/bin"
  fi
  case "$install_dir" in
    /) ;;
    */) install_dir=${install_dir%/} ;;
  esac

  detect_target
  check_tools

  # Release files are mirrored on the website under the same layout as GitHub Releases.
  # "latest" is resolved to a tag first, so an archive and its checksum always come from
  # the same release, even while a cache still holds the previous one.
  case "$version" in
    "" | latest)
      latest=$(fetch_text "$SITE/releases/latest.txt") ||
        die "could not look up the latest release at $SITE/releases/latest.txt"
      version=$(printf '%s' "$latest" | tr -d ' \r\n')
      case "$version" in
        v[0-9]*) ;;
        *) die "$SITE/releases/latest.txt does not hold a release tag" ;;
      esac
      ;;
    v*) ;;
    *) version="v$version" ;;
  esac
  base="$SITE/releases/download/$version"

  asset="say-$TARGET.tar.xz"
  info "Installing ${BOLD}say${RESET} ($version, $TARGET) into $install_dir"

  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  WORK_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t say-install)

  archive="$WORK_DIR/$asset"
  download "$base/$asset" "$archive" ||
    die "could not download $base/$asset. Check the version tag and your network connection."
  download "$base/$asset.sha256" "$archive.sha256" ||
    die "could not download $base/$asset.sha256"

  expected=$(awk 'NR == 1 { print tolower($1) }' "$archive.sha256" | tr -d '\r')
  case "$expected" in
    *[!0-9a-f]* | "") die "the checksum file for $asset is not in the expected format" ;;
  esac
  [ "${#expected}" -eq 64 ] || die "the checksum file for $asset is not in the expected format"
  actual=$(sha256_of "$archive")
  [ "$actual" = "$expected" ] ||
    die "checksum mismatch for $asset (expected $expected, got $actual). Nothing was installed."

  mkdir "$WORK_DIR/x"
  tar -xJf "$archive" -C "$WORK_DIR/x"
  binary="$WORK_DIR/x/say"
  [ -f "$binary" ] || die "$asset does not contain say"

  mkdir -p "$install_dir" 2>/dev/null ||
    die "cannot create $install_dir. Choose another folder with SAY_INSTALL_DIR."
  [ -w "$install_dir" ] ||
    die "cannot write to $install_dir. Choose another folder with SAY_INSTALL_DIR."

  # Copy next to the destination and try it there first (some systems mount
  # /tmp noexec). Then rename: the swap is atomic, so a say that is running
  # right now keeps working.
  STAGED="$install_dir/.say.install.$$"
  cp "$binary" "$STAGED"
  chmod 755 "$STAGED"

  if output=$("$STAGED" --version 2>&1); then
    starts=1
  else
    starts=0
    case "$output" in
      *GLIBC_*)
        printf '%s\n' "$output" | sed 's/^/  /' >&2
        glibc=$(ldd --version 2>/dev/null | awk 'NR == 1 { print $NF }')
        die "the prebuilt say needs a newer glibc than this system has (${glibc:-unknown}). Nothing was installed. Build it from source instead: cargo install --git https://github.com/$REPO"
        ;;
    esac
  fi

  mv -f "$STAGED" "$install_dir/say"
  STAGED=""

  if [ "$starts" -eq 0 ]; then
    warn "say was installed to $install_dir/say, but it does not start:"
    printf '%s\n' "$output" | sed 's/^/  /' >&2
    case "$output" in
      *libasound*)
        warn "say plays audio through ALSA. Install its runtime library, then run say again:"
        warn "  Debian/Ubuntu: sudo apt-get install libasound2   (libasound2t64 on Ubuntu 24.04 and later)"
        warn "  Fedora/RHEL:   sudo dnf install alsa-lib"
        warn "  Arch:          sudo pacman -S alsa-lib"
        ;;
    esac
    exit 1
  fi

  info "Installed $output to $install_dir/say"
  path_advice "$install_dir"
  info ""
  info "Try it: say \"Hello from say\""
}

main "$@"
