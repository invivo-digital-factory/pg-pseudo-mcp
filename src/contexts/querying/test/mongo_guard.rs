use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::contexts::querying::domain::constants::mongo_policy::{
    EXPRESSION_OPERATORS, EXTENDED_JSON_REFUSED, EXTENDED_JSON_TYPES, FIELD_QUERY_OPERATORS,
    GEOMETRY_OPERATORS, STAGES, TEXT_OPERATORS, TOP_LEVEL_QUERY_OPERATORS,
};
use crate::contexts::querying::domain::errors::Refusal;
use crate::contexts::querying::domain::repositories::PseudonymTranslator;
use crate::contexts::querying::domain::services::mongo_guard::{
    InspectedMongo, inspect_aggregate, inspect_find,
};
use crate::contexts::querying::domain::types::{
    AggregateRequest, FindRequest, GuardedMongo, MongoCommand, MongoReadPreference,
    PseudonymLookup, QueryLimits, Translations,
};

const PSEUDONYM: &str = "noemie.vasseur@example.net";
const REAL: &str = "ylwenna.kervadec@canary-mail.fr";
const UNKNOWN_PSEUDONYM: &str = "inconnu.total@example.org";
const NUMERIC_PSEUDONYM: i64 = 734_918_265;
const NUMERIC_REAL: &str = "418273652";

#[derive(Default)]
struct FakeStore {
    pseudonyms: HashMap<String, String>,
    lookups: AtomicUsize,
}

impl FakeStore {
    fn with(mut self, pseudonym: &str, real: &str) -> Self {
        self.pseudonyms
            .insert(pseudonym.to_string(), real.to_string());
        self
    }

    fn known() -> Self {
        Self::default()
            .with(PSEUDONYM, REAL)
            .with(&NUMERIC_PSEUDONYM.to_string(), NUMERIC_REAL)
    }

    fn lookups(&self) -> usize {
        self.lookups.load(Ordering::SeqCst)
    }
}

impl PseudonymTranslator for FakeStore {
    fn lookup(&self, text: &str) -> PseudonymLookup {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        if let Some(real) = self.pseudonyms.get(text) {
            return PseudonymLookup::Known { real: real.clone() };
        }
        let domain = text.rsplit_once('@').map(|(_, domain)| domain);
        if matches!(domain, Some("example.com" | "example.net" | "example.org")) {
            PseudonymLookup::Unknown
        } else {
            PseudonymLookup::NotAPseudonym
        }
    }
}

fn limits() -> QueryLimits {
    QueryLimits {
        max_rows: 50,
        timeout_ms: 7_000,
        ..QueryLimits::default()
    }
}

fn find(filter: Value) -> FindRequest {
    FindRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "orders".into(),
        filter: Some(filter),
        ..FindRequest::default()
    }
}

fn aggregate(pipeline: Value) -> AggregateRequest {
    AggregateRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "orders".into(),
        pipeline: pipeline.as_array().expect("a pipeline array").clone(),
        ..AggregateRequest::default()
    }
}

fn finish(inspected: InspectedMongo, store: &FakeStore) -> GuardedMongo {
    let mut translations = Translations::default();
    for literal in inspected.literals() {
        translations.record(&literal, store.lookup(&literal));
    }
    inspected.finish(&translations, &limits())
}

fn prepare_find(request: &FindRequest, store: &FakeStore) -> Result<GuardedMongo, Refusal> {
    inspect_find(request).map(|inspected| finish(inspected, store))
}

fn prepare_aggregate(
    request: &AggregateRequest,
    store: &FakeStore,
) -> Result<GuardedMongo, Refusal> {
    inspect_aggregate(request).map(|inspected| finish(inspected, store))
}

fn pipeline_of(guarded: &GuardedMongo) -> Value {
    match &guarded.command {
        MongoCommand::Aggregate(aggregate) => Value::Array(aggregate.pipeline.clone()),
        MongoCommand::Find(_) => panic!("an aggregate command"),
    }
}

fn filter_of(guarded: &GuardedMongo) -> Value {
    match &guarded.command {
        MongoCommand::Find(find) => find.filter.clone(),
        MongoCommand::Aggregate(_) => panic!("a find command"),
    }
}

fn find_refusal(filter: Value) -> Refusal {
    prepare_find(&find(filter.clone()), &FakeStore::known())
        .expect_err(&format!("refused: {filter}"))
}

fn aggregate_refusal(pipeline: Value) -> Refusal {
    prepare_aggregate(&aggregate(pipeline.clone()), &FakeStore::known())
        .expect_err(&format!("refused: {pipeline}"))
}

fn warning_codes(guarded: &GuardedMongo) -> Vec<String> {
    guarded
        .warnings
        .iter()
        .map(|warning| warning.code.clone())
        .collect()
}

fn nested_filter(levels: usize) -> Value {
    (1..levels).fold(json!({"a": 1}), |inner, _| json!({ "a": inner }))
}

mod unit {
    use super::*;

    #[test]
    fn les_lectures_passent_sans_changer_de_sens() {
        for filter in [
            json!({}),
            json!({"status": "PAID", "amount": {"$gte": 10, "$lt": 100}}),
            json!({"$or": [{"status": "PAID"}, {"items": {"$size": 2}}]}),
            json!({"items": {"$elemMatch": {"qty": {"$gt": 1}, "label": "pomme"}}}),
            json!({"tags": {"$all": ["bio", "local"]}, "code": {"$exists": true, "$type": "string"}}),
            json!({"_id": {"$oid": "6650f1c2a4b3e2d1c0b9a8f7"}, "createdAt": {"$gt": {"$date": "2026-01-01T00:00:00Z"}}}),
            json!({"amount": {"$numberDecimal": "12.50"}, "count": {"$numberLong": "42"}}),
            json!({"label": {"$regex": "^Pom", "$options": "i"}, "note": {"$not": {"$regex": "x"}}}),
            json!({"$text": {"$search": "pomme", "$language": "fr", "$caseSensitive": false}}),
            json!({"loc": {"$geoWithin": {"$centerSphere": [[2.35, 48.85], 0.01]}}}),
            json!({"loc": {"$near": {"$geometry": {"type": "Point", "coordinates": [2.35, 48.85]}, "$maxDistance": 100}}}),
            json!({"$expr": {"$gt": [{"$size": "$items"}, 2]}, "$comment": "audit"}),
            json!({"＄where": "harmless field name", "\u{200b}$where": 1, " $function": 2}),
        ] {
            let guarded = prepare_find(&find(filter.clone()), &FakeStore::known())
                .unwrap_or_else(|error| panic!("{filter}: {error:?}"));
            assert_eq!(filter_of(&guarded), filter);
            assert_eq!(guarded.translated_literals, 0, "{filter}");
        }
        for pipeline in [
            json!([{"$match": {"status": "PAID"}}, {"$group": {"_id": "$status", "n": {"$sum": 1}, "total": {"$sum": "$amount"}}}, {"$sort": {"n": -1}}]),
            json!([{"$project": {"who": {"$concat": ["$firstName", " ", "$lastName"]}, "mail": "$contact.email", "status": 1, "contact": {"city": 1}}}]),
            json!([{"$lookup": {"from": "customers", "localField": "customerId", "foreignField": "_id", "as": "buyer"}}, {"$unwind": "$buyer"}]),
            json!([{"$lookup": {"from": "customers", "let": {"id": "$customerId"}, "pipeline": [{"$match": {"$expr": {"$eq": ["$_id", "$$id"]}}}], "as": "buyer"}}]),
            json!([{"$unionWith": {"coll": "archive", "pipeline": [{"$match": {"status": "PAID"}}]}}, {"$unionWith": "archive2"}]),
            json!([{"$graphLookup": {"from": "employees", "startWith": "$managerId", "connectFromField": "managerId", "connectToField": "_id", "as": "chain", "maxDepth": 3, "restrictSearchWithMatch": {"active": true}}}]),
            json!([{"$facet": {"byStatus": [{"$sortByCount": "$status"}], "total": [{"$count": "n"}]}}]),
            json!([{"$bucket": {"groupBy": "$amount", "boundaries": [0, 100, 1000], "default": "other", "output": {"n": {"$sum": 1}}}}]),
            json!([{"$bucketAuto": {"groupBy": "$amount", "buckets": 4}}]),
            json!([{"$setWindowFields": {"partitionBy": "$status", "sortBy": {"placedAt": 1}, "output": {"running": {"$sum": "$amount", "window": {"documents": ["unbounded", "current"]}}, "rank": {"$rank": {}}}}}]),
            json!([{"$geoNear": {"near": {"type": "Point", "coordinates": [2.35, 48.85]}, "distanceField": "d", "query": {"status": "PAID"}}}, {"$limit": 5}]),
            json!([{"$documents": [{"a": 1}]}]),
            json!([{"$addFields": {"year": {"$year": "$placedAt"}}}, {"$set": {"x": {"$literal": "$notAPath"}}}, {"$unset": ["internal", "debug"]}]),
            json!([{"$replaceRoot": {"newRoot": "$contact"}}, {"$replaceWith": {"$mergeObjects": ["$$ROOT", {"k": 1}]}}]),
            json!([{"$sample": {"size": 3}}, {"$skip": 1}, {"$limit": 2}]),
            json!([{"$densify": {"field": "day", "range": {"step": 1, "unit": "day", "bounds": "full"}}}, {"$fill": {"sortBy": {"day": 1}, "output": {"amount": {"method": "linear"}, "status": {"value": "none"}}}}]),
            json!([{"$project": {"names": {"$map": {"input": "$items", "as": "item", "in": {"$toUpper": "$$item.label"}}}, "big": {"$filter": {"input": "$items", "cond": {"$gt": ["$$this.qty", 3]}}}}}]),
        ] {
            let guarded = prepare_aggregate(&aggregate(pipeline.clone()), &FakeStore::known())
                .unwrap_or_else(|error| panic!("{pipeline}: {error:?}"));
            let mut expected = pipeline.as_array().unwrap().clone();
            expected.push(json!({"$limit": 51}));
            assert_eq!(pipeline_of(&guarded), Value::Array(expected));
        }
    }

    #[test]
    fn chaque_etape_ou_operateur_refuse_de_r7_est_refuse_avec_sa_regle() {
        let stages = [
            ("$out", json!([{"$match": {}}, {"$out": "copy"}])),
            ("$merge", json!([{"$merge": {"into": "copy"}}])),
            ("$currentOp", json!([{"$currentOp": {}}])),
            ("$listSessions", json!([{"$listSessions": {}}])),
            ("$listLocalSessions", json!([{"$listLocalSessions": {}}])),
            ("$collStats", json!([{"$collStats": {"count": {}}}])),
            ("$indexStats", json!([{"$indexStats": {}}])),
            ("$planCacheStats", json!([{"$planCacheStats": {}}])),
            ("$changeStream", json!([{"$changeStream": {}}])),
            ("$listSearchIndexes", json!([{"$listSearchIndexes": {}}])),
            ("$querySettings", json!([{"$querySettings": {}}])),
            (
                "$search",
                json!([{"$search": {"text": {"query": "x", "path": "a"}}}]),
            ),
            ("$searchMeta", json!([{"$searchMeta": {}}])),
            ("$vectorSearch", json!([{"$vectorSearch": {}}])),
            ("$redact", json!([{"$redact": "$$KEEP"}])),
            ("mapReduce", json!([{"mapReduce": "orders"}])),
            ("$mapReduce", json!([{"$mapReduce": {}}])),
        ];
        for (name, pipeline) in stages {
            let refusal = aggregate_refusal(pipeline);
            assert_eq!(refusal.code, "mongo_stage_not_allowed", "{name}");
            assert!(
                refusal.message.contains(name),
                "{name}: {}",
                refusal.message
            );
        }
        let operators = [
            (
                "$where",
                aggregate(json!([{"$match": {"$where": "this.a == 1"}}])),
            ),
            (
                "$function",
                aggregate(
                    json!([{"$match": {"$expr": {"$function": {"body": "function(){return true}", "args": [], "lang": "js"}}}}]),
                ),
            ),
            (
                "$accumulator",
                aggregate(
                    json!([{"$group": {"_id": null, "x": {"$accumulator": {"init": "function(){}", "accumulate": "function(){}", "accumulateArgs": [], "merge": "function(){}", "lang": "js"}}}}]),
                ),
            ),
            (
                "$function",
                aggregate(
                    json!([{"$project": {"x": {"$function": {"body": "f", "args": [], "lang": "js"}}}}]),
                ),
            ),
        ];
        for (name, request) in operators {
            let refusal = prepare_aggregate(&request, &FakeStore::known()).expect_err(name);
            assert_eq!(refusal.code, "mongo_operator_not_allowed", "{name}");
            assert!(
                refusal.message.contains(name),
                "{name}: {}",
                refusal.message
            );
        }
    }

    #[test]
    fn ae2_un_out_en_deuxieme_etape_est_refuse_en_nommant_l_etape() {
        let refusal = aggregate_refusal(json!([{"$match": {}}, {"$out": "copy"}]));
        assert_eq!(refusal.code, "mongo_stage_not_allowed");
        assert!(refusal.message.contains("$out"), "{}", refusal.message);
        assert!(refusal.message.contains("stage 2"), "{}", refusal.message);
        assert!(refusal.hint.is_some());
    }

    #[test]
    fn ae3_where_en_filtre_et_function_dans_expr_sont_refuses() {
        let refusal = find_refusal(json!({"$where": "this.a == 1"}));
        assert_eq!(refusal.code, "mongo_operator_not_allowed");
        assert!(refusal.message.contains("$where"), "{}", refusal.message);
        let refusal =
            find_refusal(json!({"$expr": {"$function": {"body": "f", "args": [], "lang": "js"}}}));
        assert_eq!(refusal.code, "mongo_operator_not_allowed");
        assert!(refusal.message.contains("$function"), "{}", refusal.message);
    }

    #[test]
    fn ae4_un_lookup_vers_une_autre_base_est_refuse() {
        let refusal = aggregate_refusal(
            json!([{"$lookup": {"from": {"db": "admin", "coll": "system.users"}, "localField": "a", "foreignField": "b", "as": "c"}}]),
        );
        assert_eq!(refusal.code, "mongo_cross_database");
        assert!(refusal.message.contains("from"), "{}", refusal.message);
    }

    #[test]
    fn les_vecteurs_de_contournement_sont_refuses_avec_leur_regle() {
        let cases: Vec<(&str, Value, &str, &str)> = vec![
            (
                "$merge dans une branche de $facet",
                json!([{"$facet": {"a": [{"$match": {}}, {"$merge": {"into": "x"}}]}}]),
                "mongo_stage_not_allowed",
                "$merge",
            ),
            (
                "$out dans une branche de $facet",
                json!([{"$facet": {"a": [{"$out": "x"}]}}]),
                "mongo_stage_not_allowed",
                "$out",
            ),
            (
                "$merge dans $lookup.pipeline",
                json!([{"$lookup": {"from": "c", "pipeline": [{"$merge": {"into": "x"}}], "as": "r"}}]),
                "mongo_stage_not_allowed",
                "$merge",
            ),
            (
                "$out dans $unionWith.pipeline",
                json!([{"$unionWith": {"coll": "c", "pipeline": [{"$out": "x"}]}}]),
                "mongo_stage_not_allowed",
                "$out",
            ),
            (
                "$out dans une $facet d'un $lookup",
                json!([{"$lookup": {"from": "c", "pipeline": [{"$facet": {"a": [{"$out": "x"}]}}], "as": "r"}}]),
                "mongo_stage_not_allowed",
                "$out",
            ),
            (
                "$where dans $elemMatch",
                json!([{"$match": {"items": {"$elemMatch": {"qty": 1, "$where": "true"}}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$where dans $and",
                json!([{"$match": {"$and": [{"a": 1}, {"$where": "true"}]}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$where dans restrictSearchWithMatch",
                json!([{"$graphLookup": {"from": "c", "startWith": "$a", "connectFromField": "a", "connectToField": "b", "as": "r", "restrictSearchWithMatch": {"$where": "true"}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$where dans $geoNear.query",
                json!([{"$geoNear": {"near": [0, 0], "distanceField": "d", "query": {"$where": "true"}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$function dans $let",
                json!([{"$project": {"x": {"$let": {"vars": {"v": {"$function": {"body": "f", "args": [], "lang": "js"}}}, "in": "$$v"}}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$accumulator dans $bucket.output",
                json!([{"$bucket": {"groupBy": "$a", "boundaries": [0, 1], "output": {"x": {"$accumulator": {}}}}}]),
                "mongo_operator_not_allowed",
                "$accumulator",
            ),
            (
                "$accumulator dans $setWindowFields",
                json!([{"$setWindowFields": {"output": {"x": {"$accumulator": {}, "window": {"documents": [0, 0]}}}}}]),
                "mongo_operator_not_allowed",
                "$accumulator",
            ),
            (
                "$function dans le let de l'agrégation par $lookup",
                json!([{"$lookup": {"from": "c", "let": {"v": {"$function": {}}}, "pipeline": [], "as": "r"}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$code dans un filtre",
                json!([{"$match": {"a": {"$code": "function(){}"}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code avec $scope",
                json!([{"$match": {"a": {"$eq": {"$code": "f", "$scope": {}}}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code dans $literal",
                json!([{"$project": {"a": {"$literal": {"$code": "f"}}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code dans $documents",
                json!([{"$documents": [{"a": {"$code": "f"}}]}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code dans $addFields",
                json!([{"$addFields": {"a": {"$code": "f"}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code à côté d'un champ",
                json!([{"$match": {"a": {"b": 1, "$code": "f"}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$code mêlé à un $oid",
                json!([{"$match": {"a": {"$oid": "6650f1c2a4b3e2d1c0b9a8f7", "$code": "f"}}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "$codeWithScope",
                json!([{"$match": {"a": {"$eq": {"$codeWithScope": {"$code": "f", "$scope": {}}}}}}]),
                "mongo_operator_not_allowed",
                "$codeWithScope",
            ),
            (
                "$codeWithScope seul",
                json!([{"$addFields": {"a": {"$codeWithScope": "f"}}}]),
                "mongo_operator_not_allowed",
                "$codeWithScope",
            ),
            (
                "$symbol",
                json!([{"$match": {"a": {"$symbol": "s"}}}]),
                "mongo_extended_json_not_allowed",
                "$symbol",
            ),
            (
                "$dbPointer",
                json!([{"$match": {"a": {"$dbPointer": {"$ref": "c", "$id": {"$oid": "6650f1c2a4b3e2d1c0b9a8f7"}}}}}]),
                "mongo_extended_json_not_allowed",
                "$dbPointer",
            ),
            (
                "$undefined",
                json!([{"$match": {"a": {"$undefined": true}}}]),
                "mongo_extended_json_not_allowed",
                "$undefined",
            ),
            (
                "$oid mal formé",
                json!([{"$match": {"_id": {"$oid": "pas-un-oid"}}}]),
                "mongo_extended_json_invalid",
                "$oid",
            ),
            (
                "$binary hérité avec $type",
                json!([{"$match": {"b": {"$binary": "AAAA", "$type": "00"}}}]),
                "mongo_extended_json_invalid",
                "$binary",
            ),
            (
                "DBRef",
                json!([{"$match": {"owner": {"$ref": "users"}}}]),
                "mongo_operator_not_allowed",
                "$ref",
            ),
            (
                "$lookup.from objet",
                json!([{"$lookup": {"from": {"db": "other", "coll": "c"}, "pipeline": [], "as": "r"}}]),
                "mongo_cross_database",
                "from",
            ),
            (
                "$unionWith avec db",
                json!([{"$unionWith": {"db": "other", "coll": "c"}}]),
                "mongo_cross_database",
                "db",
            ),
            (
                "$graphLookup.from objet",
                json!([{"$graphLookup": {"from": {"db": "admin", "coll": "c"}, "startWith": "$a", "connectFromField": "a", "connectToField": "b", "as": "r"}}]),
                "mongo_cross_database",
                "from",
            ),
            (
                "$lookup vers system.users",
                json!([{"$lookup": {"from": "system.users", "localField": "a", "foreignField": "b", "as": "r"}}]),
                "mongo_system_namespace",
                "system",
            ),
            (
                "$lookup vers system.js",
                json!([{"$lookup": {"from": "system.js", "pipeline": [], "as": "r"}}]),
                "mongo_system_namespace",
                "system",
            ),
            (
                "$unionWith vers system.profile",
                json!([{"$unionWith": "system.profile"}]),
                "mongo_system_namespace",
                "system",
            ),
            (
                "$graphLookup vers System.views",
                json!([{"$graphLookup": {"from": "System.views", "startWith": "$a", "connectFromField": "a", "connectToField": "b", "as": "r"}}]),
                "mongo_system_namespace",
                "system",
            ),
            (
                "$lookup vers $cmd",
                json!([{"$lookup": {"from": "$cmd", "pipeline": [], "as": "r"}}]),
                "mongo_invalid_name",
                "from",
            ),
            (
                "$geoNear en deuxième étape",
                json!([{"$match": {}}, {"$geoNear": {"near": [0, 0], "distanceField": "d"}}]),
                "mongo_stage_position",
                "$geoNear",
            ),
            (
                "$geoNear dans un sous-pipeline",
                json!([{"$lookup": {"from": "c", "pipeline": [{"$geoNear": {"near": [0, 0], "distanceField": "d"}}], "as": "r"}}]),
                "mongo_stage_position",
                "$geoNear",
            ),
            (
                "$geoNear dans une $facet",
                json!([{"$facet": {"a": [{"$geoNear": {"near": [0, 0], "distanceField": "d"}}]}}]),
                "mongo_stage_position",
                "$geoNear",
            ),
            (
                "une étape cachée dans $literal",
                json!([{"$project": {"a": {"$literal": {"$out": "x"}}}}]),
                "mongo_operator_not_allowed",
                "$out",
            ),
            (
                "une étape à deux clés",
                json!([{"$match": {}, "$out": "x"}]),
                "mongo_stage_not_allowed",
                "$out",
            ),
            (
                "une étape à deux clés autorisées",
                json!([{"$match": {}, "$limit": 1}]),
                "mongo_invalid_argument",
                "stage 1",
            ),
            (
                "$out comme opérateur d'expression",
                json!([{"$project": {"a": {"$out": "x"}}}]),
                "mongo_operator_not_allowed",
                "$out",
            ),
            (
                "$merge comme opérateur de requête",
                json!([{"$match": {"a": {"$merge": 1}}}]),
                "mongo_operator_not_allowed",
                "$merge",
            ),
            (
                "$jsonSchema",
                json!([{"$match": {"$jsonSchema": {}}}]),
                "mongo_operator_not_allowed",
                "$jsonSchema",
            ),
            (
                "$meta",
                json!([{"$sort": {"score": {"$meta": "textScore"}}}]),
                "mongo_operator_not_allowed",
                "$meta",
            ),
            (
                "argument inconnu de $lookup",
                json!([{"$lookup": {"from": "c", "pipeline": [], "as": "r", "into": "x"}}]),
                "mongo_invalid_argument",
                "into",
            ),
            (
                "champ de sortie préfixé par $",
                json!([{"$project": {"$where": 1}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "objet d'opérateurs mêlé à un champ",
                json!([{"$match": {"a": {"$gt": 1, "b": 2}}}]),
                "mongo_invalid_argument",
                "a",
            ),
            (
                "NUL dans une clé",
                json!([{"$match": {"a\u{0}$where": 1}}]),
                "mongo_invalid_field_name",
                "NUL",
            ),
            (
                "$limit non entier",
                json!([{"$limit": "10"}]),
                "mongo_invalid_argument",
                "$limit",
            ),
            (
                "$limit à zéro",
                json!([{"$limit": 0}]),
                "mongo_invalid_argument",
                "$limit",
            ),
            (
                "$search hors de $text",
                json!([{"$match": {"a": {"$search": "x"}}}]),
                "mongo_operator_not_allowed",
                "$search",
            ),
            (
                "argument inconnu de $text",
                json!([{"$match": {"$text": {"$search": "x", "$where": "y"}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$function dans la liste d'un $in de $expr",
                json!([{"$match": {"$expr": {"$in": ["$a", ["x", {"$function": {}}]]}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$function comparé à un champ dans $expr",
                json!([{"$match": {"$expr": {"$eq": ["$a", {"$function": {}}]}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$where dans un $elemMatch de $all",
                json!([{"$match": {"a": {"$all": [{"$elemMatch": {"b": 1, "$where": "true"}}]}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$where sous $not",
                json!([{"$match": {"a": {"$not": {"$where": "true"}}}}]),
                "mongo_operator_not_allowed",
                "$where",
            ),
            (
                "$function dans une sortie de fenêtre",
                json!([{"$setWindowFields": {"output": {"x": {"$sum": 1, "$function": {}}}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$function dans $sortByCount",
                json!([{"$sortByCount": {"$function": {}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
            (
                "$code comme clé d'argument de $lookup",
                json!([{"$lookup": {"from": "c", "as": "r", "$code": "f"}}]),
                "mongo_extended_json_not_allowed",
                "$code",
            ),
            (
                "NUL dans un nom d'étape",
                json!([{"$match\u{0}": {}}]),
                "mongo_invalid_field_name",
                "NUL",
            ),
            (
                "$function dans le let de l'agrégation",
                json!([{"$match": {}}, {"$project": {"a": {"$let": {"vars": {"$function": 1}, "in": 1}}}}]),
                "mongo_operator_not_allowed",
                "$function",
            ),
        ];
        for (label, pipeline, code, name) in cases {
            let refusal = aggregate_refusal(pipeline);
            assert_eq!(refusal.code, code, "{label}: {}", refusal.message);
            assert!(
                refusal.message.contains(name),
                "{label}: {}",
                refusal.message
            );
        }
    }

    #[test]
    fn un_operateur_inconnu_est_refuse_avec_son_nom_et_sa_position() {
        let refusal = find_refusal(json!({"status": {"$fooBar": 1}}));
        assert_eq!(refusal.code, "mongo_operator_not_allowed");
        assert!(refusal.message.contains("$fooBar"), "{}", refusal.message);
        assert!(
            refusal.message.contains("filter.status.$fooBar"),
            "{}",
            refusal.message
        );
        let refusal =
            aggregate_refusal(json!([{"$match": {}}, {"$project": {"x": {"$fooBar": "$a"}}}]));
        assert_eq!(refusal.code, "mongo_operator_not_allowed");
        assert!(refusal.message.contains("$fooBar"), "{}", refusal.message);
        assert!(refusal.message.contains("stage 2"), "{}", refusal.message);
        assert!(
            refusal.message.contains("pipeline[1].$project.x.$fooBar"),
            "{}",
            refusal.message
        );
        let refusal = aggregate_refusal(json!([{"$fooBar": {}}]));
        assert_eq!(refusal.code, "mongo_stage_not_allowed");
        assert!(refusal.message.contains("$fooBar"), "{}", refusal.message);
    }

    #[test]
    fn les_bases_et_collections_systeme_sont_refusees() {
        for database in ["admin", "local", "config", "Admin"] {
            let request = FindRequest {
                database: Some(database.into()),
                ..find(json!({}))
            };
            let refusal = prepare_find(&request, &FakeStore::known()).expect_err(database);
            assert_eq!(refusal.code, "mongo_system_namespace", "{database}");
            assert!(refusal.hint.is_some());
        }
        let request = AggregateRequest {
            collection: "system.users".into(),
            ..aggregate(json!([]))
        };
        let refusal = prepare_aggregate(&request, &FakeStore::known()).expect_err("system.users");
        assert_eq!(refusal.code, "mongo_system_namespace");
        for collection in ["$cmd", "", "a\u{0}b"] {
            let request = FindRequest {
                collection: collection.into(),
                ..find(json!({}))
            };
            let refusal = prepare_find(&request, &FakeStore::known()).expect_err(collection);
            assert_eq!(refusal.code, "mongo_invalid_name", "{collection:?}");
        }
        let request = FindRequest {
            database: Some("sh.op".into()),
            ..find(json!({}))
        };
        let refusal = prepare_find(&request, &FakeStore::known()).expect_err("sh.op");
        assert_eq!(refusal.code, "mongo_invalid_name");
    }

    #[test]
    fn les_options_de_find_sont_verifiees() {
        let request = FindRequest {
            projection: Some(
                json!({"email": 1, "items": {"$slice": 2}, "upper": {"$toUpper": "$name"}}),
            ),
            sort: Some(json!({"placedAt": -1})),
            hint: Some(json!({"status": 1})),
            collation: Some(json!({"locale": "fr", "strength": 2})),
            skip: Some(5),
            ..find(json!({"status": "PAID"}))
        };
        let guarded = prepare_find(&request, &FakeStore::known()).expect("accepted");
        let MongoCommand::Find(command) = &guarded.command else {
            panic!("a find command");
        };
        assert_eq!(command.projection, request.projection);
        assert_eq!(command.sort, request.sort);
        assert_eq!(command.hint, request.hint);
        assert_eq!(command.collation, request.collation);
        assert_eq!(command.skip, Some(5));
        for (label, request, code) in [
            (
                "projection $function",
                FindRequest {
                    projection: Some(json!({"x": {"$function": {}}})),
                    ..find(json!({}))
                },
                "mongo_operator_not_allowed",
            ),
            (
                "projection $where",
                FindRequest {
                    projection: Some(json!({"$where": 1})),
                    ..find(json!({}))
                },
                "mongo_operator_not_allowed",
            ),
            (
                "sort $meta",
                FindRequest {
                    sort: Some(json!({"s": {"$meta": "textScore"}})),
                    ..find(json!({}))
                },
                "mongo_operator_not_allowed",
            ),
            (
                "sort texte",
                FindRequest {
                    sort: Some(json!({"s": "up"})),
                    ..find(json!({}))
                },
                "mongo_invalid_argument",
            ),
            (
                "hint $code",
                FindRequest {
                    hint: Some(json!({"a": {"$code": "f"}})),
                    ..find(json!({}))
                },
                "mongo_extended_json_not_allowed",
            ),
            (
                "collation inconnue",
                FindRequest {
                    collation: Some(json!({"locale": "fr", "$where": 1})),
                    ..find(json!({}))
                },
                "mongo_invalid_argument",
            ),
            (
                "filtre non objet",
                find(json!([1])),
                "mongo_invalid_argument",
            ),
            (
                "filtre extended JSON",
                find(json!({"$oid": "6650f1c2a4b3e2d1c0b9a8f7"})),
                "mongo_invalid_argument",
            ),
        ] {
            let refusal = prepare_find(&request, &FakeStore::known()).expect_err(label);
            assert_eq!(refusal.code, code, "{label}: {}", refusal.message);
        }
    }

    #[test]
    fn les_limites_de_taille_sont_appliquees() {
        let fifty: Vec<Value> = (0..50).map(|_| json!({"$match": {}})).collect();
        assert!(
            prepare_aggregate(&aggregate(Value::Array(fifty.clone())), &FakeStore::known()).is_ok()
        );
        let mut fifty_one = fifty;
        fifty_one.push(json!({"$match": {}}));
        let refusal = aggregate_refusal(Value::Array(fifty_one));
        assert_eq!(refusal.code, "mongo_pipeline_too_long");
        assert!(refusal.message.contains("50"), "{}", refusal.message);

        assert!(prepare_find(&find(nested_filter(32)), &FakeStore::known()).is_ok());
        let refusal = find_refusal(nested_filter(33));
        assert_eq!(refusal.code, "mongo_nesting_too_deep");
        assert!(refusal.message.contains("32"), "{}", refusal.message);
        let deep_facet = (0..31).fold(json!([]), |inner, _| json!([{"$facet": {"a": inner}}]));
        let refusal = aggregate_refusal(deep_facet);
        assert_eq!(refusal.code, "mongo_nesting_too_deep");

        let refusal = find_refusal(json!({"note": "x".repeat(70 * 1024)}));
        assert_eq!(refusal.code, "mongo_request_too_large");
        assert!(refusal.message.contains("65536"), "{}", refusal.message);
        assert!(!refusal.message.contains("xxxx"), "{}", refusal.message);

        let thousand: Vec<Value> = (0..1_000).map(Value::from).collect();
        assert!(
            prepare_find(
                &find(json!({"n": {"$in": thousand.clone()}})),
                &FakeStore::known()
            )
            .is_ok()
        );
        let mut thousand_one = thousand;
        thousand_one.push(Value::from(1_000));
        let refusal = find_refusal(json!({"n": {"$in": thousand_one}}));
        assert_eq!(refusal.code, "mongo_array_too_long");
        assert!(refusal.message.contains("1000"), "{}", refusal.message);
    }

    #[test]
    fn la_limite_de_lignes_est_ajoutee_au_pipeline_et_bornee_dans_find() {
        let guarded =
            prepare_aggregate(&aggregate(json!([{"$count": "n"}])), &FakeStore::known()).unwrap();
        assert_eq!(
            pipeline_of(&guarded),
            json!([{"$count": "n"}, {"$limit": 51}])
        );
        let guarded = prepare_aggregate(&aggregate(json!([])), &FakeStore::known()).unwrap();
        assert_eq!(pipeline_of(&guarded), json!([{"$limit": 51}]));
        assert_eq!(guarded.execution.max_time_ms, 7_000);
        assert!(!guarded.execution.allow_disk_use);
        assert_eq!(
            guarded.execution.read_preference,
            MongoReadPreference::SecondaryPreferred
        );
        assert_eq!(guarded.execution.batch_size, 51);
        assert_eq!(guarded.execution.max_documents, 51);
        assert_eq!(guarded.execution.comment, "scarif");

        let request = FindRequest {
            limit: Some(1_000),
            ..find(json!({}))
        };
        let guarded = prepare_find(&request, &FakeStore::known()).unwrap();
        let MongoCommand::Find(command) = &guarded.command else {
            panic!("find")
        };
        assert_eq!(command.limit, 51);
        assert_eq!(guarded.clamps.len(), 1);
        assert_eq!(guarded.clamps[0].code, "limit_clamped");
        assert_eq!(guarded.clamps[0].requested, 1_000);
        assert_eq!(guarded.clamps[0].applied, 50);

        for (requested, applied) in [(None, 51), (Some(0), 51), (Some(10), 10), (Some(51), 51)] {
            let request = FindRequest {
                limit: requested,
                ..find(json!({}))
            };
            let guarded = prepare_find(&request, &FakeStore::known()).unwrap();
            let MongoCommand::Find(command) = &guarded.command else {
                panic!("find")
            };
            assert_eq!(command.limit, applied, "{requested:?}");
            assert!(guarded.clamps.is_empty(), "{requested:?}");
        }
    }

    #[test]
    fn ae6_les_pseudonymes_compares_a_un_champ_sont_traduits() {
        let cases = [
            (
                json!({"customer.email": PSEUDONYM}),
                json!({"customer.email": REAL}),
                1,
            ),
            (
                json!({"email": {"$eq": PSEUDONYM}}),
                json!({"email": {"$eq": REAL}}),
                1,
            ),
            (
                json!({"email": {"$ne": PSEUDONYM}}),
                json!({"email": {"$ne": REAL}}),
                1,
            ),
            (
                json!({"email": {"$in": [PSEUDONYM, "autre"]}}),
                json!({"email": {"$in": [REAL, "autre"]}}),
                1,
            ),
            (
                json!({"email": {"$nin": [PSEUDONYM]}}),
                json!({"email": {"$nin": [REAL]}}),
                1,
            ),
            (
                json!({"emails": {"$all": [PSEUDONYM]}}),
                json!({"emails": {"$all": [REAL]}}),
                1,
            ),
            (
                json!({"contacts": {"$elemMatch": {"email": PSEUDONYM}}}),
                json!({"contacts": {"$elemMatch": {"email": REAL}}}),
                1,
            ),
            (
                json!({"emails": {"$elemMatch": {"$eq": PSEUDONYM}}}),
                json!({"emails": {"$elemMatch": {"$eq": REAL}}}),
                1,
            ),
            (
                json!({"email": {"$not": {"$eq": PSEUDONYM}}}),
                json!({"email": {"$not": {"$eq": REAL}}}),
                1,
            ),
            (
                json!({"$or": [{"email": PSEUDONYM}, {"backup": PSEUDONYM}]}),
                json!({"$or": [{"email": REAL}, {"backup": REAL}]}),
                2,
            ),
            (
                json!({"$expr": {"$eq": ["$contact.email", PSEUDONYM]}}),
                json!({"$expr": {"$eq": ["$contact.email", REAL]}}),
                1,
            ),
            (
                json!({"$expr": {"$ne": [PSEUDONYM, "$contact.email"]}}),
                json!({"$expr": {"$ne": [REAL, "$contact.email"]}}),
                1,
            ),
            (
                json!({"$expr": {"$in": ["$contact.email", [PSEUDONYM]]}}),
                json!({"$expr": {"$in": ["$contact.email", [REAL]]}}),
                1,
            ),
            (
                json!({"$expr": {"$in": [PSEUDONYM, "$emails"]}}),
                json!({"$expr": {"$in": [REAL, "$emails"]}}),
                1,
            ),
        ];
        for (filter, expected, count) in cases {
            let guarded = prepare_find(&find(filter.clone()), &FakeStore::known())
                .unwrap_or_else(|error| panic!("{filter}: {error:?}"));
            assert_eq!(filter_of(&guarded), expected, "{filter}");
            assert_eq!(guarded.translated_literals, count, "{filter}");
            assert!(
                guarded
                    .injected
                    .iter()
                    .all(|pair| pair == &(REAL.to_string(), PSEUDONYM.to_string())),
                "{filter}"
            );
            assert!(
                guarded.warnings.is_empty(),
                "{filter}: {:?}",
                guarded.warnings
            );
        }
    }

    #[test]
    fn les_sous_pipelines_et_branches_traduisent_aussi() {
        let pipeline = json!([
            {"$match": {"email": PSEUDONYM}},
            {"$lookup": {"from": "c", "pipeline": [{"$match": {"email": PSEUDONYM}}], "as": "r"}},
            {"$unionWith": {"coll": "c", "pipeline": [{"$match": {"email": {"$in": [PSEUDONYM]}}}]}},
            {"$facet": {"a": [{"$match": {"email": PSEUDONYM}}]}},
            {"$graphLookup": {"from": "c", "startWith": "$a", "connectFromField": "a", "connectToField": "b", "as": "r", "restrictSearchWithMatch": {"email": PSEUDONYM}}},
            {"$geoNear": {"near": [0, 0], "distanceField": "d", "query": {"email": PSEUDONYM}}}
        ]);
        let refusal = aggregate_refusal(pipeline.clone());
        assert_eq!(refusal.code, "mongo_stage_position");
        let mut stages = pipeline.as_array().unwrap().clone();
        stages.pop();
        let guarded =
            prepare_aggregate(&aggregate(Value::Array(stages)), &FakeStore::known()).unwrap();
        assert_eq!(guarded.translated_literals, 5);
        let text = pipeline_of(&guarded).to_string();
        assert!(!text.contains(PSEUDONYM), "{text}");
    }

    #[test]
    fn ae7_un_pseudonyme_hors_comparaison_reste_tel_quel_avec_un_avertissement() {
        for filter in [
            json!({"contact.email": {"$regex": format!("^{PSEUDONYM}")}}),
            json!({"contact.email": {"$gt": PSEUDONYM}}),
            json!({"$expr": {"$eq": [{"$concat": ["$a", PSEUDONYM]}, "x"]}}),
            json!({"contact": {"email": PSEUDONYM}}),
            json!({"$text": {"$search": PSEUDONYM}}),
        ] {
            let guarded = prepare_find(&find(filter.clone()), &FakeStore::known()).unwrap();
            assert_eq!(filter_of(&guarded), filter);
            assert_eq!(guarded.translated_literals, 0, "{filter}");
            assert!(guarded.injected.is_empty(), "{filter}");
            assert_eq!(
                warning_codes(&guarded),
                ["pseudonym_not_translated"],
                "{filter}"
            );
            assert!(guarded.warnings[0].message.contains(PSEUDONYM));
            assert!(!guarded.warnings[0].message.contains(REAL));
        }
        let pipeline = json!([{"$project": {"tag": {"$concat": ["$a", PSEUDONYM]}}}]);
        let guarded = prepare_aggregate(&aggregate(pipeline), &FakeStore::known()).unwrap();
        assert_eq!(guarded.translated_literals, 0);
        assert_eq!(warning_codes(&guarded), ["pseudonym_not_translated"]);

        let guarded = prepare_find(
            &find(json!({"email": UNKNOWN_PSEUDONYM})),
            &FakeStore::known(),
        )
        .unwrap();
        assert_eq!(filter_of(&guarded), json!({"email": UNKNOWN_PSEUDONYM}));
        assert_eq!(warning_codes(&guarded), ["unresolved_pseudonym"]);
    }

    #[test]
    fn r18_un_pseudonyme_numerique_redevient_un_nombre_si_la_valeur_reelle_est_en_chiffres() {
        let guarded = prepare_find(
            &find(json!({"phone": NUMERIC_PSEUDONYM})),
            &FakeStore::known(),
        )
        .unwrap();
        assert_eq!(filter_of(&guarded), json!({"phone": 418_273_652}));
        assert_eq!(guarded.translated_literals, 1);
        let store = FakeStore::default().with("123456", "AB-12");
        let guarded = prepare_find(&find(json!({"code": {"$in": [123_456]}})), &store).unwrap();
        assert_eq!(filter_of(&guarded), json!({"code": {"$in": ["AB-12"]}}));
        let guarded = prepare_find(&find(json!({"code": "123456"})), &store).unwrap();
        assert_eq!(filter_of(&guarded), json!({"code": "AB-12"}));
        let guarded = prepare_find(
            &find(json!({"phone": {"$gt": NUMERIC_PSEUDONYM}})),
            &FakeStore::known(),
        )
        .unwrap();
        assert_eq!(guarded.translated_literals, 0);
        assert_eq!(warning_codes(&guarded), ["pseudonym_not_translated"]);
    }

    #[test]
    fn une_valeur_reelle_qui_commence_par_un_dollar_reste_une_valeur_dans_une_expression() {
        let store = FakeStore::default().with(PSEUDONYM, "$ROOT");
        let guarded = prepare_find(
            &find(json!({"$expr": {"$eq": ["$email", PSEUDONYM]}})),
            &store,
        )
        .unwrap();
        assert_eq!(
            filter_of(&guarded),
            json!({"$expr": {"$eq": ["$email", {"$literal": "$ROOT"}]}})
        );
        let guarded = prepare_find(&find(json!({"email": PSEUDONYM})), &store).unwrap();
        assert_eq!(filter_of(&guarded), json!({"email": "$ROOT"}));
    }

    #[test]
    fn r10_un_refus_ne_reprend_aucune_valeur_de_la_requete() {
        let canary = "ylwenna.kervadec@canary-mail.fr";
        let refusals = [
            find_refusal(
                json!({"email": canary, "$where": "this.email == 'ylwenna.kervadec@canary-mail.fr'"}),
            ),
            find_refusal(json!({canary: {"$where": "x"}})),
            find_refusal(json!({"email": {"$fooBar": canary}})),
            find_refusal(json!({"email": {"$code": canary}})),
            find_refusal(json!({"note": format!("{canary}{}", "x".repeat(70 * 1024))})),
            aggregate_refusal(json!([{"$match": {"email": canary}}, {"$out": canary}])),
            aggregate_refusal(
                json!([{"$lookup": {"from": format!("system.{canary}"), "pipeline": [], "as": "r"}}]),
            ),
            aggregate_refusal(
                json!([{"$lookup": {"from": {"db": canary, "coll": canary}, "as": "r"}}]),
            ),
            aggregate_refusal(json!([{format!("${canary}"): {}}])),
            aggregate_refusal(json!([{"$project": {"a": {format!("${canary}"): 1}}}])),
        ];
        for refusal in refusals {
            let text = format!("{} {} {:?}", refusal.code, refusal.message, refusal.hint);
            for fragment in ["ylwenna", "kervadec", "canary-mail"] {
                assert!(!text.to_lowercase().contains(fragment), "{text}");
            }
        }
    }

    #[test]
    fn un_refus_n_interroge_jamais_le_magasin_de_pseudonymes() {
        let store = FakeStore::known();
        let request = aggregate(json!([{"$match": {"email": PSEUDONYM}}, {"$out": "copy"}]));
        assert!(inspect_aggregate(&request).is_err());
        let request = find(json!({"email": PSEUDONYM, "$where": "true"}));
        let refused = inspect_find(&request).map(|inspected| finish(inspected, &store));
        assert!(refused.is_err());
        assert_eq!(store.lookups(), 0);
        let accepted = find(json!({"email": PSEUDONYM}));
        let guarded = prepare_find(&accepted, &store).unwrap();
        assert_eq!(guarded.translated_literals, 1);
        assert!(store.lookups() > 0);
    }

    #[test]
    fn le_let_de_l_agregation_est_verifie_comme_une_expression() {
        let request = AggregateRequest {
            let_vars: Some(json!({"seuil": {"$add": [1, 2]}})),
            ..aggregate(json!([{"$match": {"$expr": {"$gt": ["$amount", "$$seuil"]}}}]))
        };
        let guarded = prepare_aggregate(&request, &FakeStore::known()).expect("accepted");
        let MongoCommand::Aggregate(command) = &guarded.command else {
            panic!("an aggregate command");
        };
        assert_eq!(command.let_vars, request.let_vars);
        for let_vars in [json!({"v": {"$function": {}}}), json!({"$where": 1})] {
            let request = AggregateRequest {
                let_vars: Some(let_vars.clone()),
                ..aggregate(json!([]))
            };
            let refusal = prepare_aggregate(&request, &FakeStore::known()).expect_err("refused");
            assert_eq!(refusal.code, "mongo_operator_not_allowed", "{let_vars}");
        }
    }

    #[test]
    fn une_cle_dupliquee_garde_la_derniere_valeur_vue_par_la_garde_et_la_commande() {
        let filter: Value = serde_json::from_str(r#"{"a": {"$where": "true"}, "a": 1}"#).unwrap();
        assert_eq!(filter, json!({"a": 1}));
        let guarded = prepare_find(&find(filter), &FakeStore::known()).unwrap();
        assert_eq!(filter_of(&guarded), json!({"a": 1}));
        let filter: Value = serde_json::from_str(r#"{"a": 1, "a": {"$where": "true"}}"#).unwrap();
        assert_eq!(find_refusal(filter).code, "mongo_operator_not_allowed");
        let stage: Value =
            serde_json::from_str(r#"{"$match": {}, "$match": {"$where": "1"}}"#).unwrap();
        assert_eq!(
            aggregate_refusal(Value::Array(vec![stage])).code,
            "mongo_operator_not_allowed"
        );
    }

    #[test]
    fn les_listes_autorisees_ne_contiennent_ni_operateur_dangereux_ni_type_executable() {
        let refused = [
            "$out",
            "$merge",
            "$where",
            "$function",
            "$accumulator",
            "$currentOp",
            "$listSessions",
            "$listLocalSessions",
            "$collStats",
            "$indexStats",
            "$planCacheStats",
            "$changeStream",
            "$search",
            "$searchMeta",
            "$vectorSearch",
            "$listSearchIndexes",
            "$querySettings",
            "$code",
            "$codeWithScope",
            "$jsonSchema",
            "$meta",
        ];
        let stage_and_operator_lists = [
            STAGES,
            TOP_LEVEL_QUERY_OPERATORS,
            FIELD_QUERY_OPERATORS,
            GEOMETRY_OPERATORS,
            EXPRESSION_OPERATORS,
        ];
        for list in stage_and_operator_lists {
            for name in refused {
                assert!(!list.contains(&name), "{name}");
            }
            for name in EXTENDED_JSON_TYPES.iter().chain(EXTENDED_JSON_REFUSED) {
                assert!(!list.contains(name), "{name}");
            }
        }
        assert!(
            !TEXT_OPERATORS
                .iter()
                .any(|name| STAGES.contains(name) || EXPRESSION_OPERATORS.contains(name))
        );
        assert_eq!(STAGES.len(), 26);
        assert_eq!(
            TOP_LEVEL_QUERY_OPERATORS.len() + FIELD_QUERY_OPERATORS.len() + TEXT_OPERATORS.len(),
            33
        );
        assert_eq!(GEOMETRY_OPERATORS.len(), 7);
        assert_eq!(EXPRESSION_OPERATORS.len(), 143);
        let mut unique = EXPRESSION_OPERATORS.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), EXPRESSION_OPERATORS.len());
    }
}
