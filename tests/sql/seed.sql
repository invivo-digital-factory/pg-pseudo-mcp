CREATE ROLE pgpseudo LOGIN PASSWORD 'pgpseudo';
GRANT ALL ON DATABASE pgpseudo TO pgpseudo;
ALTER DATABASE pgpseudo OWNER TO pgpseudo;

SET ROLE pgpseudo;

CREATE TYPE order_status AS ENUM ('pending', 'paid', 'shipped');
CREATE TYPE payment_method AS ENUM ('CASH', 'SEPA', 'BANK_CARD');

CREATE TABLE customers (
    id integer PRIMARY KEY,
    prenom text NOT NULL,
    nom text NOT NULL,
    email text NOT NULL UNIQUE,
    telephone text,
    telephone_secours bigint,
    iban text,
    siren text,
    nir text,
    date_naissance date,
    ville text,
    profile jsonb,
    emails_secondaires text[],
    avatar bytea,
    created_at timestamptz NOT NULL DEFAULT '2026-01-15 10:00:00+00'
);

COMMENT ON COLUMN customers.email IS 'Primary contact, e.g. ylwenna.kervadec@canary-mail.fr';

CREATE FUNCTION customer_name(customer integer) RETURNS text
    LANGUAGE sql STABLE
    AS $$ SELECT prenom || ' ' || nom FROM customers WHERE id = customer $$;

CREATE TABLE orders (
    id integer PRIMARY KEY,
    customer_id integer NOT NULL REFERENCES customers(id),
    titulaire text,
    status order_status NOT NULL,
    payment_method payment_method NOT NULL,
    status_label text NOT NULL,
    amount numeric(12, 4) NOT NULL,
    placed_at timestamptz NOT NULL
);

CREATE INDEX orders_customer_id_idx ON orders (customer_id);

CREATE TABLE notes (
    id integer PRIMARY KEY,
    customer_id integer NOT NULL REFERENCES customers(id),
    body text NOT NULL
);

CREATE SEQUENCE audit_seq;

INSERT INTO customers (id, prenom, nom, email, telephone, telephone_secours, iban, siren, nir, date_naissance, ville, profile, emails_secondaires, avatar) VALUES
  (1, 'Ylwenna', 'Kervadec', 'ylwenna.kervadec@canary-mail.fr', '06 41 27 83 95', 33641278395, 'FR76 3000 4000 0312 3456 7890 143', '418273652', '2851175123456 22', '1985-11-02', 'Plouzané',
   '{"contact": {"mail": "o.hazvekmor@canary-mail.fr", "age": 42}, "kervadec.ylwenna@canary-mail.fr": 3}',
   ARRAY['ylwenna.k@canary-mail.fr', 'yk.pro@canary-mail.fr'], '\x89504e47'),
  (2, 'Odrigan', 'Hazvekmor', 'odrigan.hazvekmor@canary-mail.fr', '07 58 14 92 36', 33758149236, 'FR14 2004 1010 0505 0001 3M02 606', '418273652', NULL, '1972-04-19', 'Saint-Brevin',
   '{"contact": {"mail": "odrigan.h@canary-mail.fr", "age": 54}}',
   ARRAY['o.hazvekmor@canary-mail.fr'], NULL),
  (3, 'Maëlouz', 'Brizolier', 'maelouz.brizolier@canary-mail.fr', NULL, NULL, NULL, NULL, NULL, NULL, 'Quimperlé',
   NULL, NULL, NULL);

INSERT INTO orders (id, customer_id, titulaire, status, payment_method, status_label, amount, placed_at) VALUES
  (10, 1, 'Ylwenna Kervadec', 'paid', 'CASH', 'PAID', 12345.6700, '2026-03-01 09:30:00+00'),
  (11, 1, 'Kervadec', 'shipped', 'SEPA', 'en_cours', 99.9000, '2026-03-05 14:00:00+00'),
  (12, 2, 'Odrigan Hazvekmor', 'pending', 'BANK_CARD', 'PENDING', 15.0000, '2026-04-11 08:15:00+00'),
  (13, 3, '', 'pending', 'CASH', 'PENDING', 0.0000, '2026-04-12 08:15:00+00');

INSERT INTO notes (id, customer_id, body) VALUES
  (100, 1, 'Appel de M. Brizolier au 06 52 18 74 30, livraison 12 rue des Quenvrelles'),
  (101, 2, 'Client prefers email odrigan.hazvekmor@canary-mail.fr, IBAN FR76 3000 4000 0312 3456 7890 143');

CREATE TABLE products (
    id integer PRIMARY KEY,
    product_name text NOT NULL,
    supplier_ref text NOT NULL,
    status order_status NOT NULL
);

INSERT INTO products (id, product_name, supplier_ref, status) VALUES
  (1, 'Garance Bleue', 'lot-42', 'paid'),
  (2, 'Garance zivrel.quanteb@canary-mail.fr', 'lot-43', 'shipped');

CREATE TABLE big_numbers AS SELECT g AS id FROM generate_series(1, 10000) AS g;

RESET ROLE;
