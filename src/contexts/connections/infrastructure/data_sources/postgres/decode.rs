use std::error::Error;
use std::net::IpAddr;

use postgres_protocol::types as wire;
use postgres_types::{Field, FromSql, Kind, Type};
use tokio_postgres::Row;
use tokio_postgres::fallible_iterator::FallibleIterator;

use super::numeric::numeric_to_string;
use super::temporal;
use crate::contexts::connections::domain::types::Cell;

const BOOL: u32 = 16;
const CHAR: u32 = 18;
const NAME: u32 = 19;
const INT8: u32 = 20;
const INT2: u32 = 21;
const INT4: u32 = 23;
const TEXT: u32 = 25;
const OID: u32 = 26;
const JSON: u32 = 114;
const CIDR: u32 = 650;
const FLOAT4: u32 = 700;
const FLOAT8: u32 = 701;
const UNKNOWN: u32 = 705;
const INET: u32 = 869;
const BPCHAR: u32 = 1042;
const VARCHAR: u32 = 1043;
const DATE: u32 = 1082;
const TIME: u32 = 1083;
const TIMESTAMP: u32 = 1114;
const TIMESTAMPTZ: u32 = 1184;
const INTERVAL: u32 = 1186;
const TIMETZ: u32 = 1266;
const NUMERIC: u32 = 1700;
const RECORD: u32 = 2249;
const UUID: u32 = 2950;
const JSONB: u32 = 3802;
const JSONB_VERSION: u8 = 1;
const TEXT_EXTENSION_TYPES: &[&str] = &["citext"];

type Decoded = Result<Cell, String>;

struct RawValue<'a>(Option<&'a [u8]>);

impl<'a> FromSql<'a> for RawValue<'a> {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(Self(Some(raw)))
    }

    fn from_sql_null(_: &Type) -> Result<Self, Box<dyn Error + Sync + Send>> {
        Ok(Self(None))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

pub(crate) fn decode_row(row: &Row) -> Result<Vec<Cell>, String> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let raw: RawValue<'_> = row.try_get(index).map_err(|error| error.to_string())?;
            decode_value(column.type_(), raw.0).map_err(|reason| {
                format!(
                    "column {} of type {}: {reason}",
                    column.name(),
                    column.type_().name()
                )
            })
        })
        .collect()
}

pub(crate) fn decode_value(ty: &Type, raw: Option<&[u8]>) -> Decoded {
    match raw {
        None => Ok(Cell::Null),
        Some(raw) => decode_present(ty, raw),
    }
}

fn decode_present(ty: &Type, raw: &[u8]) -> Decoded {
    match ty.kind() {
        Kind::Array(member) => return decode_array(member, raw),
        Kind::Composite(fields) => return decode_record(raw, Some(fields)),
        Kind::Domain(base) => return decode_present(base, raw),
        Kind::Enum(_) => return text(raw),
        _ => {}
    }
    match ty.oid() {
        BOOL => Ok(Cell::Bool(wire::bool_from_sql(raw).map_err(reason)?)),
        INT2 => Ok(Cell::Int(i64::from(
            wire::int2_from_sql(raw).map_err(reason)?,
        ))),
        INT4 => Ok(Cell::Int(i64::from(
            wire::int4_from_sql(raw).map_err(reason)?,
        ))),
        INT8 => Ok(Cell::Int(wire::int8_from_sql(raw).map_err(reason)?)),
        OID => Ok(Cell::Int(i64::from(
            wire::oid_from_sql(raw).map_err(reason)?,
        ))),
        FLOAT4 => Ok(Cell::Float(f64::from(
            wire::float4_from_sql(raw).map_err(reason)?,
        ))),
        FLOAT8 => Ok(Cell::Float(wire::float8_from_sql(raw).map_err(reason)?)),
        NUMERIC => Ok(Cell::Numeric(numeric_to_string(raw)?)),
        TEXT | VARCHAR | BPCHAR | NAME | CHAR | UNKNOWN => text(raw),
        JSON => json(raw),
        JSONB => match raw.split_first() {
            Some((&JSONB_VERSION, body)) => json(body),
            _ => Err("unsupported jsonb version".into()),
        },
        UUID => Ok(Cell::Uuid(uuid(wire::uuid_from_sql(raw).map_err(reason)?))),
        DATE => Ok(Cell::Date(temporal::date_to_string(
            wire::date_from_sql(raw).map_err(reason)?,
        ))),
        TIME => Ok(Cell::Time(temporal::time_to_string(
            wire::time_from_sql(raw).map_err(reason)?,
        ))),
        TIMETZ => timetz(raw),
        TIMESTAMP => Ok(Cell::Timestamp(temporal::timestamp_to_string(
            wire::timestamp_from_sql(raw).map_err(reason)?,
            false,
        ))),
        TIMESTAMPTZ => Ok(Cell::TimestampTz(temporal::timestamp_to_string(
            wire::timestamp_from_sql(raw).map_err(reason)?,
            true,
        ))),
        INTERVAL => interval(raw),
        INET | CIDR => inet(raw, ty.oid() == CIDR),
        RECORD => decode_record(raw, None),
        _ if TEXT_EXTENSION_TYPES.contains(&ty.name()) => text(raw),
        _ => Ok(opaque(ty.name(), raw)),
    }
}

fn reason(error: Box<dyn Error + Sync + Send>) -> String {
    error.to_string()
}

fn opaque(type_name: &str, raw: &[u8]) -> Cell {
    Cell::Opaque {
        type_name: type_name.to_string(),
        len: raw.len(),
    }
}

fn text(raw: &[u8]) -> Decoded {
    Ok(Cell::Text(
        wire::text_from_sql(raw).map_err(reason)?.to_string(),
    ))
}

fn json(raw: &[u8]) -> Decoded {
    serde_json::from_slice(raw)
        .map(Cell::Json)
        .map_err(|error| format!("invalid json: {error}"))
}

fn uuid(bytes: [u8; 16]) -> String {
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn timetz(raw: &[u8]) -> Decoded {
    let mut reader = Reader(raw);
    let micros = reader.i64()?;
    let zone = reader.i32()?;
    reader.finish()?;
    Ok(Cell::Time(temporal::timetz_to_string(micros, zone)))
}

fn interval(raw: &[u8]) -> Decoded {
    let mut reader = Reader(raw);
    let micros = reader.i64()?;
    let days = reader.i32()?;
    let months = reader.i32()?;
    reader.finish()?;
    Ok(Cell::Interval(temporal::interval_to_string(
        micros, days, months,
    )))
}

fn inet(raw: &[u8], always_masked: bool) -> Decoded {
    let value = wire::inet_from_sql(raw).map_err(reason)?;
    let full = match value.addr() {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if always_masked || value.netmask() != full {
        Ok(Cell::Inet(format!("{}/{}", value.addr(), value.netmask())))
    } else {
        Ok(Cell::Inet(value.addr().to_string()))
    }
}

fn decode_array(member: &Type, raw: &[u8]) -> Decoded {
    let array = wire::array_from_sql(raw).map_err(reason)?;
    let mut dimensions = Vec::new();
    let mut dimension_iter = array.dimensions();
    while let Some(dimension) = FallibleIterator::next(&mut dimension_iter).map_err(reason)? {
        dimensions.push(usize::try_from(dimension.len).map_err(|_| "negative array dimension")?);
    }
    let mut cells = Vec::new();
    let mut values = array.values();
    while let Some(value) = FallibleIterator::next(&mut values).map_err(reason)? {
        cells.push(decode_value(member, value)?);
    }
    if dimensions.is_empty() {
        return if cells.is_empty() {
            Ok(Cell::Array(Vec::new()))
        } else {
            Err("array without dimensions has elements".into())
        };
    }
    if dimensions.iter().product::<usize>() != cells.len() {
        return Err("array dimensions do not match its elements".into());
    }
    Ok(nest(cells, &dimensions))
}

fn nest(cells: Vec<Cell>, dimensions: &[usize]) -> Cell {
    match dimensions {
        [] | [_] => Cell::Array(cells),
        [outer, inner @ ..] => {
            let chunk: usize = inner.iter().product();
            let mut remaining = cells.into_iter();
            Cell::Array(
                (0..*outer)
                    .map(|_| nest(remaining.by_ref().take(chunk).collect(), inner))
                    .collect(),
            )
        }
    }
}

fn decode_record(raw: &[u8], fields: Option<&[Field]>) -> Decoded {
    let mut reader = Reader(raw);
    let count =
        usize::try_from(reader.i32()?).map_err(|_| "negative record field count".to_string())?;
    if fields.is_some_and(|fields| fields.len() != count) {
        return Err("record field count does not match its type".into());
    }
    let mut decoded = Vec::with_capacity(count);
    for index in 0..count {
        let oid = reader.u32()?;
        let len = reader.i32()?;
        let value = match usize::try_from(len) {
            Ok(len) => Some(reader.take(len)?),
            Err(_) => None,
        };
        let field = match fields {
            Some(fields) => (
                fields[index].name().to_string(),
                decode_value(fields[index].type_(), value)?,
            ),
            None => (format!("f{}", index + 1), anonymous_field(oid, value)?),
        };
        decoded.push(field);
    }
    reader.finish()?;
    Ok(Cell::Composite(decoded))
}

fn anonymous_field(oid: u32, value: Option<&[u8]>) -> Decoded {
    match (Type::from_oid(oid), value) {
        (_, None) => Ok(Cell::Null),
        (Some(ty), value) => decode_value(&ty, value),
        (None, Some(raw)) => Ok(opaque(&oid.to_string(), raw)),
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        if self.0.len() < len {
            return Err("truncated value".into());
        }
        let (head, tail) = self.0.split_at(len);
        self.0 = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let bytes = self.take(N)?;
        let mut out = [0; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn i32(&mut self) -> Result<i32, String> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    fn finish(&self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("trailing bytes after value".into())
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn record(fields: &[(u32, Option<&[u8]>)]) -> Vec<u8> {
        let mut buf = (fields.len() as i32).to_be_bytes().to_vec();
        for (oid, value) in fields {
            buf.extend_from_slice(&oid.to_be_bytes());
            match value {
                Some(bytes) => {
                    buf.extend_from_slice(&(bytes.len() as i32).to_be_bytes());
                    buf.extend_from_slice(bytes);
                }
                None => buf.extend_from_slice(&(-1i32).to_be_bytes()),
            }
        }
        buf
    }

    #[test]
    fn les_types_inconnus_deviennent_opaques_avec_leur_longueur() {
        let cell = decode_value(&Type::BYTEA, Some(&[1, 2, 3])).unwrap();
        assert_eq!(
            cell,
            Cell::Opaque {
                type_name: "bytea".into(),
                len: 3
            }
        );
    }

    #[test]
    fn les_enregistrements_anonymes_decodent_les_champs_natifs_et_masquent_les_inconnus() {
        let int = 7i32.to_be_bytes();
        let raw = record(&[(INT4, Some(&int)), (99_999, Some(b"abc")), (TEXT, None)]);
        let cell = decode_value(&Type::RECORD, Some(&raw)).unwrap();
        assert_eq!(
            cell,
            Cell::Composite(vec![
                ("f1".into(), Cell::Int(7)),
                (
                    "f2".into(),
                    Cell::Opaque {
                        type_name: "99999".into(),
                        len: 3
                    }
                ),
                ("f3".into(), Cell::Null),
            ])
        );
    }

    #[test]
    fn les_enregistrements_tronques_sont_rejetes() {
        let mut raw = record(&[(TEXT, Some(b"abc"))]);
        raw.pop();
        assert!(decode_value(&Type::RECORD, Some(&raw)).is_err());
    }

    #[test]
    fn jsonb_exige_l_octet_de_version_connu() {
        assert_eq!(
            decode_value(&Type::JSONB, Some(b"\x01{\"a\":1}")).unwrap(),
            Cell::Json(serde_json::json!({"a": 1}))
        );
        assert!(decode_value(&Type::JSONB, Some(b"\x02{}")).is_err());
    }

    #[test]
    fn les_uuid_sont_rendus_en_minuscules_avec_tirets() {
        let bytes = [
            0xa0, 0xee, 0xbc, 0x99, 0x9c, 0x0b, 0x4e, 0xf8, 0xbb, 0x6d, 0x6b, 0xb9, 0xbd, 0x38,
            0x0a, 0x11,
        ];
        assert_eq!(uuid(bytes), "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11");
    }
}
