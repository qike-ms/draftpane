#!/bin/sh
set -eu

REPOSITORY="qike-ms/draftpane"
VERSION="latest"
INSTALL_DIR="${HOME}/.local/bin"
UNINSTALL=0

usage() {
    cat <<'EOF'
Install a checksummed DraftPane release binary without Cargo.

Usage: ./install.sh [--version TAG] [--install-dir DIR] [--uninstall]

Options:
  --version TAG      Install an immutable release such as v0.1.1 (default: latest)
  --install-dir DIR  Destination directory (default: ~/.local/bin)
  --uninstall        Remove DraftPane from the destination directory
  -h, --help         Show this help
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || { echo "Error: --version requires a tag" >&2; exit 2; }
            VERSION=$2
            shift 2
            ;;
        --install-dir)
            [ "$#" -ge 2 ] || { echo "Error: --install-dir requires a directory" >&2; exit 2; }
            INSTALL_DIR=$2
            shift 2
            ;;
        --uninstall)
            UNINSTALL=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Error: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

case "$INSTALL_DIR" in
    /*) ;;
    *) echo "Error: --install-dir must be an absolute path" >&2; exit 2 ;;
esac

DESTINATION="${INSTALL_DIR}/draftpane"
if [ "$UNINSTALL" -eq 1 ]; then
    if [ -e "$DESTINATION" ] || [ -L "$DESTINATION" ]; then
        rm -f -- "$DESTINATION"
        echo "Removed $DESTINATION"
    else
        echo "DraftPane is not installed at $DESTINATION"
    fi
    exit 0
fi

for command in curl tar awk mktemp; do
    command -v "$command" >/dev/null 2>&1 || {
        echo "Error: required command not found: $command" >&2
        exit 1
    }
done

OS=$(uname -s)
ARCH=$(uname -m)
case "${OS}:${ARCH}" in
    Darwin:arm64) PLATFORM="Darwin-arm64" ;;
    Darwin:x86_64) PLATFORM="Darwin-x86_64" ;;
    Linux:x86_64) PLATFORM="Linux-x86_64" ;;
    Linux:aarch64|Linux:arm64) PLATFORM="Linux-arm64" ;;
    *) echo "Error: unsupported platform ${OS}/${ARCH}" >&2; exit 1 ;;
esac

ASSET="draftpane-${PLATFORM}.tar.gz"
if [ "$VERSION" = "latest" ]; then
    BASE_URL="https://github.com/${REPOSITORY}/releases/latest/download"
else
    case "$VERSION" in
        v[0-9]*.[0-9]*.[0-9]*) ;;
        *) echo "Error: invalid release tag: $VERSION" >&2; exit 2 ;;
    esac
    BASE_URL="https://github.com/${REPOSITORY}/releases/download/${VERSION}"
fi

TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/draftpane-install.XXXXXX")
STAGED=""
cleanup() {
    [ -z "$STAGED" ] || rm -f -- "$STAGED"
    rm -rf -- "$TMP_DIR"
}
trap cleanup EXIT HUP INT TERM

curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
    --output "${TMP_DIR}/${ASSET}" "${BASE_URL}/${ASSET}"
curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
    --output "${TMP_DIR}/SHA256SUMS" "${BASE_URL}/SHA256SUMS"

EXPECTED=$(awk -v asset="$ASSET" '$2 == asset || $2 == "*" asset { print $1; exit }' "${TMP_DIR}/SHA256SUMS")
[ -n "$EXPECTED" ] || { echo "Error: release checksum is missing for $ASSET" >&2; exit 1; }

if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "${TMP_DIR}/${ASSET}" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "${TMP_DIR}/${ASSET}" | awk '{print $1}')
else
    echo "Error: sha256sum or shasum is required" >&2
    exit 1
fi

[ "$ACTUAL" = "$EXPECTED" ] || {
    echo "Error: checksum verification failed for $ASSET" >&2
    exit 1
}

ARCHIVE_ENTRIES=$(tar -tzf "${TMP_DIR}/${ASSET}")
[ "$ARCHIVE_ENTRIES" = "draftpane" ] || {
    echo "Error: unexpected release archive contents" >&2
    exit 1
}
tar -xzf "${TMP_DIR}/${ASSET}" -C "$TMP_DIR"
if [ ! -f "${TMP_DIR}/draftpane" ] || [ -L "${TMP_DIR}/draftpane" ]; then
    echo "Error: release archive has no regular draftpane binary" >&2
    exit 1
fi
mkdir -p -- "$INSTALL_DIR"
chmod 0755 "${TMP_DIR}/draftpane"

STAGED=$(mktemp "${INSTALL_DIR}/.draftpane.new.XXXXXX")
cp "${TMP_DIR}/draftpane" "$STAGED"
chmod 0755 "$STAGED"
mv -f "$STAGED" "$DESTINATION"
STAGED=""
trap - EXIT HUP INT TERM
cleanup

echo "Installed DraftPane at $DESTINATION"
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "Add $INSTALL_DIR to PATH to run: draftpane <file.md>" ;;
esac
