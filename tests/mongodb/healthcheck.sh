#!/bin/bash
set -euo pipefail

test -f /tmp/seed-done
mongosh --quiet \
  "mongodb://atlas-admin:atlas-admin-test-pw@pl-0.atlas.scarif.test:1024,pl-0.atlas.scarif.test:1025,pl-0.atlas.scarif.test:1026/admin?replicaSet=rs0" \
  --eval '
    const members = rs.status().members;
    const primaries = members.filter((member) => member.stateStr === "PRIMARY");
    const secondaries = members.filter((member) => member.stateStr === "SECONDARY");
    const shop = db.getSiblingDB("shop");
    const seeded = ["customers", "orders", "products", "events", "customer_contacts"].every(
      (name) => shop[name].countDocuments({}) > 0,
    );
    const view = shop.getCollectionInfos({ name: "customer_contacts", type: "view" }).length === 1;
    if (members.length !== 3 || primaries.length !== 1 || secondaries.length !== 2 || !seeded || !view) {
      quit(1);
    }'
