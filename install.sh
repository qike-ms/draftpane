#!/bin/sh
set -eu

# Use only standard system tools, ignore shell startup injection, and make curl
# ignore per-user configuration. This also makes the embedded updater behavior
# independent of the caller's PATH.
PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH
unset ENV BASH_ENV CDPATH

REPOSITORY="qike-ms/draftpane"
VERSION="latest"
INSTALL_DIR=""
MINIMUM_VERSION=""
UNINSTALL=0

usage() {
    cat <<'EOF'
Install a checksummed DraftPane release binary without Cargo.

Usage: ./install.sh [--version TAG] [--install-dir DIR] [--uninstall]

Options:
  --version TAG      Install an immutable release such as v0.1.1 (default: latest)
  --install-dir DIR  Destination directory (default: ~/.local/bin)
  --minimum-version VERSION
                     Refuse an older release (used by `draftpane update`)
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
        --minimum-version)
            [ "$#" -ge 2 ] || { echo "Error: --minimum-version requires a version" >&2; exit 2; }
            MINIMUM_VERSION=$2
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

if [ -z "$INSTALL_DIR" ]; then
    [ -n "${HOME:-}" ] || { echo "Error: HOME is not set; pass --install-dir" >&2; exit 2; }
    INSTALL_DIR="${HOME}/.local/bin"
fi

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

for command in curl tar awk cat mktemp; do
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

curl --disable --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
    --connect-timeout 15 --max-time 300 --max-filesize 52428800 \
    --output "${TMP_DIR}/${ASSET}" "${BASE_URL}/${ASSET}"
curl --disable --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
    --connect-timeout 15 --max-time 60 --max-filesize 1048576 \
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
[ "$ARCHIVE_ENTRIES" = "draftpane
VERSION" ] || {
    echo "Error: unexpected release archive contents" >&2
    exit 1
}
ARCHIVE_DETAILS=$(tar -tvzf "${TMP_DIR}/${ASSET}")
printf '%s\n' "$ARCHIVE_DETAILS" | awk '
    substr($0, 1, 1) != "-" { exit 1 }
    END { if (NR != 2) exit 1 }
' || {
    echo "Error: release archive entries must be regular files" >&2
    exit 1
}
tar -xzf "${TMP_DIR}/${ASSET}" -C "$TMP_DIR"
if [ ! -f "${TMP_DIR}/draftpane" ] || [ -L "${TMP_DIR}/draftpane" ] || \
   [ ! -f "${TMP_DIR}/VERSION" ] || [ -L "${TMP_DIR}/VERSION" ]; then
    echo "Error: release archive is missing required regular files" >&2
    exit 1
fi

CANDIDATE_VERSION=$(cat "${TMP_DIR}/VERSION")
case "$CANDIDATE_VERSION" in
    ''|*[!0-9.]*|.*|*.|*..*)
        echo "Error: release archive has an invalid version" >&2
        exit 1
        ;;
esac
if [ "$VERSION" != "latest" ] && [ "v${CANDIDATE_VERSION}" != "$VERSION" ]; then
    echo "Error: release archive version does not match the requested tag" >&2
    exit 1
fi
if [ -n "$MINIMUM_VERSION" ]; then
    case "$MINIMUM_VERSION" in
        ''|*[!0-9.]*|.*|*.|*..*)
            echo "Error: invalid minimum version" >&2
            exit 2
            ;;
    esac
    awk -v candidate="$CANDIDATE_VERSION" -v minimum="$MINIMUM_VERSION" '
        function valid(value, parts) {
            return value ~ /^[0-9]+\.[0-9]+\.[0-9]+$/ && split(value, parts, ".") == 3
        }
        BEGIN {
            if (!valid(candidate, c) || !valid(minimum, m)) exit 2
            for (i = 1; i <= 3; i++) {
                if ((c[i] + 0) > (m[i] + 0)) exit 0
                if ((c[i] + 0) < (m[i] + 0)) exit 1
            }
            exit 0
        }
    ' || {
        result=$?
        if [ "$result" -eq 1 ]; then
            echo "Error: refusing to install an older DraftPane release" >&2
        else
            echo "Error: release archive has an invalid version" >&2
        fi
        exit 1
    }
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
