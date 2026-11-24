const admin = db.getSiblingDB("admin");
const shop = db.getSiblingDB("shop");

admin.createUser({
  user: "atlas-reader",
  pwd: "atlas-reader-test-pw",
  roles: [{ role: "read", db: "shop" }],
});
admin.createUser({
  user: "atlas-writer",
  pwd: "atlas-writer-test-pw",
  roles: [{ role: "readWrite", db: "shop" }],
});

const ylwenna = ObjectId("65a000000000000000000001");
const odrigan = ObjectId("65a000000000000000000002");
const maelouz = ObjectId("65a000000000000000000003");

shop.customers.insertMany([
  {
    _id: ylwenna,
    firstName: "Ylwenna",
    lastName: "Kervadec",
    contact: {
      email: "ylwenna.kervadec@canary-mail.fr",
      phones: ["06 41 27 83 95", "33641278395"],
      secondaryEmails: ["ylwenna.k@canary-mail.fr", "yk.pro@canary-mail.fr"],
    },
    telephone: NumberLong("33641278395"),
    iban: "FR76 3000 4000 0312 3456 7890 143",
    birthDate: ISODate("1985-11-02T00:00:00Z"),
    note: "Appel de M. Brizolier au 06 52 18 74 30, livraison 12 rue des Lilas",
    preferences: {
      "kervadec.ylwenna@canary-mail.fr": { newsletter: true },
      "o.hazvekmor@canary-mail.fr": { newsletter: false },
    },
  },
  {
    _id: odrigan,
    firstName: "Odrigan",
    lastName: "Hazvekmor",
    contact: {
      email: "odrigan.hazvekmor@canary-mail.fr",
      phones: ["07 58 14 92 36", "33758149236"],
      secondaryEmails: ["odrigan.h@canary-mail.fr", "o.hazvekmor@canary-mail.fr"],
    },
    telephone: NumberLong("33758149236"),
    iban: "FR14 2004 1010 0505 0001 3M02 606",
    birthDate: ISODate("1972-04-19T00:00:00Z"),
    note: "Client prefers email odrigan.hazvekmor@canary-mail.fr, IBAN FR76 3000 4000 0312 3456 7890 143",
    preferences: {},
  },
  {
    _id: maelouz,
    firstName: "Maëlouz",
    lastName: "Brizolier",
    contact: { email: "maelouz.brizolier@canary-mail.fr", phones: [] },
    note: "",
    preferences: {},
  },
]);

shop.customers.createIndex({ "contact.email": 1 });

shop.orders.insertMany([
  {
    _id: 10,
    customer: { _id: ylwenna, email: "ylwenna.kervadec@canary-mail.fr" },
    holder: "Ylwenna Kervadec",
    status: "paid",
    items: [
      { product: "Garance Bleue", quantity: NumberLong("2") },
      { product: "Garance zivrel.quanteb@canary-mail.fr", quantity: NumberLong("1") },
    ],
    placedAt: ISODate("2026-03-01T09:30:00Z"),
  },
  {
    _id: 11,
    customer: { _id: ylwenna, email: "ylwenna.kervadec@canary-mail.fr" },
    holder: "Kervadec",
    status: "shipped",
    items: [{ product: "Garance Bleue", quantity: NumberLong("5") }],
    placedAt: ISODate("2026-03-05T14:00:00Z"),
  },
  {
    _id: 12,
    customer: { _id: odrigan, email: "odrigan.hazvekmor@canary-mail.fr" },
    holder: "Odrigan Hazvekmor",
    status: "pending",
    items: [{ product: "Garance Bleue", quantity: NumberLong("1") }],
    placedAt: ISODate("2026-04-11T08:15:00Z"),
  },
]);

shop.products.insertMany([
  { _id: 1, name: "Garance Bleue", supplierRef: "lot-42", status: "paid" },
  { _id: 2, name: "Garance zivrel.quanteb@canary-mail.fr", supplierRef: "lot-43", status: "shipped" },
]);

shop.createView("customer_contacts", "customers", [
  { $project: { firstName: 1, lastName: 1, "contact.email": 1, status: { $literal: "active" } } },
]);

shop.events.insertMany([
  {
    _id: 1,
    kind: "payment",
    amount: NumberDecimal("12345.6700"),
    telephone: NumberLong("33641278395"),
    quantity: NumberLong("2"),
    payload: BinData(0, "iVBORw0K"),
    token: UUID("0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"),
    pattern: /^FR76 3000/i,
    seenAt: Timestamp(1767225600, 1),
    at: ISODate("2026-03-01T09:30:00Z"),
    ratio: 0.25,
    flag: true,
    missing: null,
  },
  {
    _id: 2,
    kind: "refund",
    amount: NumberDecimal("-15.0000"),
    telephone: NumberLong("33758149236"),
    quantity: NumberLong("1"),
    payload: BinData(0, "AAECAwQ="),
    token: UUID("7a8b9c0d-1e2f-4a3b-9c4d-5e6f7a8b9c0d"),
    pattern: /kervadec/,
    seenAt: Timestamp(1767312000, 2),
    at: ISODate("2026-04-11T08:15:00Z"),
    ratio: 1.5,
    flag: false,
    missing: null,
  },
]);
