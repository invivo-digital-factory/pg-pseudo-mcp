#!/bin/bash
set -euo pipefail

HOST=pl-0.atlas.scarif.test
PORTS="1024 1025 1026"
ADMIN_USER=atlas-admin
ADMIN_PASSWORD=atlas-admin-test-pw
KEYFILE=/etc/mongo-test-keyfile

head -c 756 /dev/urandom | base64 -w 0 > "$KEYFILE"
chmod 400 "$KEYFILE"

rm -f /tmp/seed-done

PIDS=()
for port in $PORTS; do
  mkdir -p "/data/test/$port"
  mongod --replSet rs0 --bind_ip_all --port "$port" --dbpath "/data/test/$port" \
    --keyFile "$KEYFILE" --quiet --logpath "/var/log/mongod-$port.log" --wiredTigerCacheSizeGB 0.25 &
  PIDS+=($!)
  tail -F "/var/log/mongod-$port.log" 2>/dev/null | sed -u "s/^/mongod-$port: /" &
done

wait_for() {
  local port=$1 expression=$2
  for _ in $(seq 1 120); do
    if [ "$(mongosh --quiet --port "$port" --eval "$expression" 2>/dev/null || true)" = "true" ]; then
      return 0
    fi
    sleep 1
  done
  echo "timeout waiting on port $port for: $expression" >&2
  return 1
}

initialize() {
  for port in $PORTS; do
    wait_for "$port" "db.hello().ok === 1"
  done
  mongosh --quiet --port 1024 --eval "
    rs.initiate({
      _id: 'rs0',
      members: [
        { _id: 0, host: '$HOST:1024' },
        { _id: 1, host: '$HOST:1025' },
        { _id: 2, host: '$HOST:1026' }
      ]
    })"
  local primary=""
  for _ in $(seq 1 120); do
    primary=$(mongosh --quiet --port 1024 --eval "db.hello().primary || ''" 2>/dev/null || true)
    [ -n "$primary" ] && break
    sleep 1
  done
  [ -n "$primary" ] || { echo "no primary elected" >&2; return 1; }
  mongosh --quiet --port "${primary##*:}" --eval "
    db.getSiblingDB('admin').createUser({
      user: '$ADMIN_USER', pwd: '$ADMIN_PASSWORD', roles: [{ role: 'root', db: 'admin' }]
    })"
  mongosh --quiet \
    "mongodb://$ADMIN_USER:$ADMIN_PASSWORD@$HOST:1024,$HOST:1025,$HOST:1026/admin?replicaSet=rs0" \
    --file /seed/seed.js
  touch /tmp/seed-done
}

MAIN=$$
(
  set +e
  ( set -e; initialize )
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "replica set initialization failed with status $status" >&2
    kill "$MAIN"
  fi
) &
INIT=$!
wait -n "${PIDS[@]}"
kill "$INIT" 2>/dev/null || true
