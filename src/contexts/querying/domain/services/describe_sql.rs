pub fn describe_sql(oid: i64) -> String {
    let names_of = |array: &str, relation: &str| {
        format!(
            "(SELECT coalesce(json_agg(a.attname ORDER BY k.ord), '[]'::json) \
              FROM unnest({array}) WITH ORDINALITY AS k(attnum, ord) \
              JOIN pg_attribute a ON a.attrelid = {relation} AND a.attnum = k.attnum)"
        )
    };
    let own_columns = names_of("con.conkey", "con.conrelid");
    let referenced_columns = names_of("con.confkey", "con.confrelid");
    format!(
        "SELECT n.nspname::text, c.relname::text, c.relkind::text, c.reltuples::float8, \
           obj_description(c.oid, 'pg_class'), \
           (SELECT json_agg(json_build_object('name', a.attname, 'type', format_type(a.atttypid, a.atttypmod), \
                'not_null', a.attnotnull, 'comment', col_description(c.oid, a.attnum)) ORDER BY a.attnum) \
            FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped), \
           (SELECT {own_columns} FROM pg_constraint con WHERE con.conrelid = c.oid AND con.contype = 'p'), \
           (SELECT json_agg(json_build_object('columns', {own_columns}, 'ref_schema', rn.nspname, \
                'ref_table', rc.relname, 'ref_columns', {referenced_columns}) ORDER BY con.conname) \
            FROM pg_constraint con JOIN pg_class rc ON rc.oid = con.confrelid \
            JOIN pg_namespace rn ON rn.oid = rc.relnamespace \
            WHERE con.conrelid = c.oid AND con.contype = 'f'), \
           (SELECT json_agg(json_build_object('schema', sn.nspname, 'table', sc.relname, 'columns', {own_columns}) \
                ORDER BY sn.nspname, sc.relname, con.conname) \
            FROM pg_constraint con JOIN pg_class sc ON sc.oid = con.conrelid \
            JOIN pg_namespace sn ON sn.oid = sc.relnamespace \
            WHERE con.confrelid = c.oid AND con.contype = 'f'), \
           (SELECT json_agg(json_build_object('name', ic.relname, 'unique', i.indisunique, \
                'partial', i.indpred IS NOT NULL, 'columns', \
                (SELECT coalesce(json_agg(a.attname ORDER BY k.ord), '[]'::json) \
                 FROM unnest(i.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
                 LEFT JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum \
                 WHERE k.ord <= i.indnkeyatts)) ORDER BY ic.relname) \
            FROM pg_index i JOIN pg_class ic ON ic.oid = i.indexrelid WHERE i.indrelid = c.oid) \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.oid = {oid}::oid"
    )
}
