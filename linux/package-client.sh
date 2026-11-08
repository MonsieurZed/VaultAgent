#!/usr/bin/env bash
# Sur Maison : fichiers publics uniquement, runtime Node provenant de l'image epinglee.
set -euo pipefail
destination=/srv/stack/vaultagent/data/downloads
source=/srv/stack/vaultagent/source/remote-client
stage=$(mktemp -d /tmp/vaultagent-client-package.XXXXXXXX)
case "$stage" in /tmp/vaultagent-client-package.*) ;; *) exit 1 ;; esac
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$destination" "$stage/remote-client"
cp "$source"/*.mjs "$source/README.md" "$stage/remote-client/"
docker cp vaultagent:/usr/local/bin/node "$stage/node"
docker cp vaultagent:/usr/local/LICENSE "$stage/LICENSE-NODE.txt"
chmod 755 "$stage/node"
cat > "$stage/vaultagent-client" <<'SCRIPT'
#!/bin/sh
set -eu
directory=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$directory/node" "$directory/remote-client/cli.mjs" "$@"
SCRIPT
chmod 755 "$stage/vaultagent-client"
archive=VaultAgent-Linux-Client-3.0.0-preview.2.tar.gz
tar -czf "$destination/$archive" -C "$stage" node LICENSE-NODE.txt vaultagent-client remote-client
(cd "$destination" && sha256sum "$archive" > "$archive.sha256")
echo 'LINUX_CLIENT_PACKAGED; runtime-included; no-private-data'
