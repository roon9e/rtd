#!/bin/sh
# Resolve the edge's RSA identity onto the mounted state directory before
# starting the edge, mirroring the gramsrv v2 edge entrypoint. The private key
# lives on that volume; it is never left in the repository, the image, or a
# bind mount.
#
# Environment:
#   RTD_EDGE_STATE_DIR           state volume (default /var/lib/telesrv-edge)
#   RTD_EDGE_RSA_IDENTITY_MODE   `test` (decode the embedded test key) or
#                                `generated` (create one); default `generated`
#   RTD_EDGE_TEST_KEY            base64 PEM for `test` mode
#   RTD_EDGE_RSA_KEY             output: the resolved PEM path is exported
set -eu

state_dir="${RTD_EDGE_STATE_DIR:-/var/lib/telesrv-edge}"
key="$state_dir/server_rsa.pem"
mode="$(printf '%s' "${RTD_EDGE_RSA_IDENTITY_MODE:-generated}" | tr '[:upper:]' '[:lower:]')"
embedded_key="${RTD_EDGE_TEST_KEY:-/usr/share/telesrv/keys/test-server-rsa.pem.b64}"

mkdir -p "$state_dir"

if [ ! -s "$key" ]; then
  tmp="$state_dir/.server_rsa.pem.$$"
  trap 'rm -f "$tmp"' EXIT HUP INT TERM
  case "$mode" in
    test)
      if [ ! -r "$embedded_key" ]; then
        echo "rtd-edge: test RSA identity requested but $embedded_key is not readable" >&2
        exit 66
      fi
      # Tolerate a CRLF checkout of the base64 asset.
      tr -d '\r' < "$embedded_key" | base64 -d > "$tmp"
      ;;
    generated)
      openssl genrsa -out "$tmp" 2048 2>/dev/null
      ;;
    *)
      echo "rtd-edge: RTD_EDGE_RSA_IDENTITY_MODE must be 'test' or 'generated', got '$mode'" >&2
      exit 64
      ;;
  esac
  chmod 0400 "$tmp"
  mv "$tmp" "$key"
  trap - EXIT HUP INT TERM
fi

RTD_EDGE_RSA_KEY="$key"
export RTD_EDGE_RSA_KEY
exec /usr/local/bin/rtd-edge "$@"
