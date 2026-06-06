#!/bin/sh
# gage installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/gageml/gage/main/scripts/install.sh | sh
#
# Environment overrides:
#   GAGE_VERSION       install a specific tag (default: latest release, prereleases included)
#   GAGE_INSTALL_DIR   install directory   (default: $XDG_BIN_HOME, else ~/.local/bin)
#
# Installs the `gage` binary for the current platform as a non-root user.

set -eu

REPO="gageml/gage"
BIN="gage"

need() {
    for cmd in "$@"; do
        command -v "$cmd" >/dev/null 2>&1 || err "required command not found: $cmd"
    done
}

err() {
    printf 'error: %s\n' "$*" >&2; exit 1;
}

info() {
    printf '%s\n' "$*" >&2;
}

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"
    case "${os}/${arch}" in
        Linux/x86_64 | Linux/amd64) echo "x86_64-unknown-linux-gnu" ;;
        *) err "unsupported platform: ${os} ${arch}" ;;
    esac
}

# Raw HTTPS GET to stdout, for metadata and the checksums file itself.
get() {
    curl -fsSL "$1"
}

latest_version() {
    get "https://api.github.com/repos/${REPO}/releases" \
        | grep -m1 '"tag_name"' \
        | sed -E 's/.*"tag_name" *: *"([^"]+)".*/\1/'
}

# Download $archive and its checksums from $base_url into $tmp, then verify the
# archive against SHA256SUMS. Returns only when ${tmp}/${archive} is a
# checksum-verified copy; otherwise aborts.
get_archive() {
    base_url="$1"
    archive="$2"
    tmp="$3"

    info "Downloading $archive"
    get "${base_url}/${archive}" > "${tmp}/${archive}"
    get "${base_url}/SHA256SUMS" > "${tmp}/SHA256SUMS"

    info "Verifying archive"
    expected="$(awk -v f="$archive" '$2 == f || $2 == "*" f {print $1}' "${tmp}/SHA256SUMS")"
    [ -n "$expected" ] || err "no checksum for ${archive} in SHA256SUMS"
    printf '%s  %s\n' "$expected" "${tmp}/${archive}" | sha256sum -c - >/dev/null 2>&1 \
        || err "checksum mismatch for ${archive}"
}

check_path() {
    dir="$1"
    case ":${PATH}:" in
        *":${dir}:"*) ;;
        *) info "note: ${dir} is not on your PATH; add it to use ${BIN} directly" ;;
    esac
}

main() {
    need curl tar sha256sum

    target="$(detect_target)"
    version="${GAGE_VERSION:-$(latest_version)}"
    install_dir="${GAGE_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"
    archive="${BIN}-${version}-${target}.tar.gz"
    base_url="https://github.com/${REPO}/releases/download/${version}"

    info "installing ${BIN} ${version} (${target}) to ${install_dir}"

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    get_archive "$base_url" "$archive" "$tmp"
    tar -C "$tmp" -xzf "${tmp}/${archive}"

    info "Installing $BIN to ${install_dir}"
    mkdir -p "$install_dir"
    install -m 0755 "${tmp}/${BIN}" "${install_dir}/${BIN}"
    check_path "$install_dir"
}

main "$@"
