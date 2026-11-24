#!/bin/bash
set -euo pipefail

HOST=pl-0.atlas.scarif.test
PORTS="1024 1025 1026"
ADMIN_USER=atlas-admin
ADMIN_PASSWORD=atlas-admin-test-pw
KEYFILE=/etc/mongo-test-keyfile

head -c 756 /dev/urandom | base64 -w 0 > "$KEYFILE"
chmod 400 "$KEYFILE"

PIDS=()
for port in $PORTS; do
  mkdir -p "/data/test/$port"
  mongod --replSet rs0 --bind_ip_all --port "$port" --dbpath "/data/test/$port" \
    --keyFile "$KEYFILE" --quiet --logpath "/var/log/mongod-$port.log" &
  PIDS+=($!)
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
  wait_for 1024 "db.hello().isWritablePrimary"
  mongosh --quiet --port 1024 --eval "
    db.getSiblingDB('admin').createUser({
      user: '$ADMIN_USER', pwd: '$ADMIN_PASSWORD', roles: [{ role: 'root', db: 'admin' }]
    })"
  mongosh --quiet \
    "mongodb://$ADMIN_USER:$ADMIN_PASSWORD@$HOST:1024,$HOST:1025,$HOST:1026/admin?replicaSet=rs0" \
    --file /seed/seed.js
  touch /tmp/seed-done
}

initialize &
INIT=$!
wait -n "${PIDS[@]}"
kill "$INIT" 2>/dev/null || true
