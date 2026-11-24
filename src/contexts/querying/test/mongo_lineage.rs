use serde_json::{Value, json};

use crate::contexts::querying::domain::services::mongo_guard::{inspect_aggregate, inspect_find};
use crate::contexts::querying::domain::services::mongo_lineage::{
    DocumentLineage, clearance, document_lineage,
};
use crate::contexts::querying::domain::types::{
    AggregateRequest, ColumnPolicy, FieldLineage, FieldSource, FindRequest, LineageClearance,
    MongoFieldPattern, PolicyDecision, QueryLimits, Translations,
};

fn field(collection: &str, path: &str) -> FieldSource {
    FieldSource {
        database: "shop".into(),
        collection: collection.into(),
        path: path.into(),
    }
}

fn source(collection: &str, path: &str) -> FieldLineage {
    FieldLineage::Source(field(collection, path))
}

fn derived(inputs: &[&str], operators: &[&str]) -> FieldLineage {
    FieldLineage::Derived {
        inputs: inputs.iter().map(|path| field("customers", path)).collect(),
        operators: operators.iter().map(|name| name.to_string()).collect(),
    }
}

fn aggregate(pipeline: Value) -> DocumentLineage {
    let request = AggregateRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "customers".into(),
        pipeline: pipeline.as_array().expect("a pipeline array").clone(),
        let_vars: Some(json!({"tier": "gold"})),
        ..AggregateRequest::default()
    };
    let guarded = inspect_aggregate(&request)
        .unwrap_or_else(|refusal| panic!("refused: {} {}", refusal.code, refusal.message))
        .finish(&Translations::default(), &QueryLimits::default());
    document_lineage(&guarded, "shop")
}

fn find(projection: Option<Value>) -> DocumentLineage {
    let request = FindRequest {
        connection: "MONGO".into(),
        database: Some("shop".into()),
        collection: "customers".into(),
        projection,
        ..FindRequest::default()
    };
    let guarded = inspect_find(&request)
        .unwrap_or_else(|refusal| panic!("refused: {} {}", refusal.code, refusal.message))
        .finish(&Translations::default(), &QueryLimits::default());
    document_lineage(&guarded, "shop")
}

fn policy(entries: &[(&str, PolicyDecision)]) -> ColumnPolicy {
    let mut policy = ColumnPolicy::default();
    for (path, decision) in entries {
        policy.insert_field(
            MongoFieldPattern {
                database: Some("shop".into()),
                collection: Some("customers".into()),
                path: path.to_lowercase(),
            },
            decision.clone(),
        );
    }
    policy
}

fn clear_on(paths: &[&str]) -> ColumnPolicy {
    let entries: Vec<(&str, PolicyDecision)> = paths
        .iter()
        .map(|path| (*path, PolicyDecision::Clear))
        .collect();
    policy(&entries)
}

const TAINTED: LineageClearance = LineageClearance::Derived {
    proven_clear: false,
};

const PROVEN_CLEAR: LineageClearance = LineageClearance::Derived { proven_clear: true };

#[test]
fn project_keeps_renamed_and_included_paths_and_taints_a_concatenation_of_names() {
    let lineage = aggregate(json!([{"$project": {
        "who": {"$concat": ["$firstName", " ", "$lastName"]},
        "mail": "$contact.email",
        "status": 1
    }}]));
    let policy = clear_on(&["status"]);

    assert_eq!(
        lineage.of("who"),
        derived(&["firstName", "lastName"], &["$concat"])
    );
    assert_eq!(clearance(&lineage.of("who"), &policy), TAINTED);
    assert_eq!(lineage.of("mail"), source("customers", "contact.email"));
    assert_eq!(lineage.of("status"), source("customers", "status"));
    assert_eq!(
        clearance(&lineage.of("status"), &policy),
        LineageClearance::Source(field("customers", "status"))
    );
    assert_eq!(lineage.of("_id"), source("customers", "_id"));
}

#[test]
fn project_excluding_a_path_keeps_the_others_as_source() {
    let lineage = aggregate(json!([{"$project": {"contact": 0}}]));

    assert_eq!(lineage.of("name"), source("customers", "name"));
    assert_eq!(lineage.of("contact.email"), FieldLineage::Unknown);
}

#[test]
fn project_nested_specification_includes_and_renames_below_a_field() {
    let lineage = aggregate(json!([{"$project": {
        "_id": 0,
        "contact": {"email": 1, "label": "$name"}
    }}]));

    assert_eq!(
        lineage.of("contact.email"),
        source("customers", "contact.email")
    );
    assert_eq!(lineage.of("contact.label"), source("customers", "name"));
    assert_eq!(lineage.of("contact.phone"), FieldLineage::Unknown);
    assert_eq!(lineage.of("_id"), FieldLineage::Unknown);
}

#[test]
fn project_literal_is_a_constant_proven_clear() {
    let lineage = aggregate(json!([{"$project": {"label": "VIP"}}]));

    assert_eq!(lineage.of("label"), derived(&[], &[]));
    assert_eq!(
        clearance(&lineage.of("label"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn add_fields_keeps_the_document_and_reads_fields_set_by_an_earlier_stage() {
    let lineage = aggregate(json!([
        {"$set": {"shout": {"$toUpper": "$email"}, "contact.alias": "$name"}},
        {"$addFields": {"copy": "$shout"}}
    ]));

    assert_eq!(lineage.of("email"), source("customers", "email"));
    assert_eq!(lineage.of("copy"), derived(&["email"], &["$toUpper"]));
    assert_eq!(lineage.of("contact.alias"), source("customers", "name"));
    assert_eq!(
        lineage.of("contact.phone"),
        source("customers", "contact.phone")
    );
}

#[test]
fn unset_removes_the_named_paths() {
    let lineage = aggregate(json!([{"$unset": ["email", "contact.phone"]}]));

    assert_eq!(lineage.of("email"), FieldLineage::Unknown);
    assert_eq!(lineage.of("contact.phone"), FieldLineage::Unknown);
    assert_eq!(
        lineage.of("contact.city"),
        source("customers", "contact.city")
    );
}

#[test]
fn group_key_keeps_its_source_and_a_sum_is_clear_only_when_its_input_is_clear_by_policy() {
    let lineage = aggregate(json!([{"$group": {
        "_id": "$status",
        "total": {"$sum": "$amount"},
        "orders": {"$sum": 1}
    }}]));

    assert_eq!(lineage.of("_id"), source("customers", "status"));
    assert_eq!(lineage.of("total"), derived(&["amount"], &["$sum"]));
    assert_eq!(
        clearance(&lineage.of("total"), &clear_on(&["amount"])),
        PROVEN_CLEAR
    );
    assert_eq!(
        clearance(&lineage.of("total"), &ColumnPolicy::default()),
        TAINTED
    );
    assert_eq!(
        clearance(&lineage.of("orders"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
    assert_eq!(lineage.of("name"), FieldLineage::Unknown);
}

#[test]
fn group_pass_through_accumulators_keep_the_source_and_others_are_derived() {
    let lineage = aggregate(json!([{"$group": {
        "_id": {"city": "$contact.city"},
        "emails": {"$push": "$email"},
        "first": {"$first": {"name": "$name"}},
        "top": {"$max": "$email"}
    }}]));

    assert_eq!(lineage.of("_id.city"), source("customers", "contact.city"));
    assert_eq!(lineage.of("emails"), source("customers", "email"));
    assert_eq!(lineage.of("first.name"), source("customers", "name"));
    assert_eq!(lineage.of("top"), derived(&["email"], &["$max"]));
    assert_eq!(
        clearance(&lineage.of("top"), &ColumnPolicy::default()),
        TAINTED
    );
}

#[test]
fn lookup_maps_the_joined_documents_to_the_foreign_collection() {
    let lineage = aggregate(json!([{"$lookup": {
        "from": "users",
        "localField": "buyerId",
        "foreignField": "_id",
        "as": "buyer"
    }}]));

    assert_eq!(lineage.of("buyer.email"), source("users", "email"));
    assert_eq!(lineage.of("name"), source("customers", "name"));
}

#[test]
fn lookup_with_a_sub_pipeline_follows_it_on_the_foreign_collection() {
    let lineage = aggregate(json!([{"$lookup": {
        "from": "users",
        "let": {"buyer": "$buyerId"},
        "pipeline": [
            {"$match": {"$expr": {"$eq": ["$_id", "$$buyer"]}}},
            {"$project": {"email": 1, "tag": "$$buyer"}}
        ],
        "as": "buyer"
    }}]));

    assert_eq!(lineage.of("buyer.email"), source("users", "email"));
    assert_eq!(lineage.of("buyer.tag"), FieldLineage::Unknown);
    assert_eq!(lineage.of("buyer.phone"), FieldLineage::Unknown);
}

#[test]
fn graph_lookup_maps_the_found_documents_to_the_foreign_collection() {
    let lineage = aggregate(json!([{"$graphLookup": {
        "from": "users",
        "startWith": "$managerId",
        "connectFromField": "managerId",
        "connectToField": "_id",
        "as": "chain",
        "depthField": "depth"
    }}]));

    assert_eq!(lineage.of("chain.email"), source("users", "email"));
    assert_eq!(
        clearance(&lineage.of("chain.depth"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn replace_root_with_a_path_re_roots_the_sources() {
    for stage in [
        json!({"$replaceRoot": {"newRoot": "$contact"}}),
        json!({"$replaceWith": "$contact"}),
    ] {
        let lineage = aggregate(json!([stage]));

        assert_eq!(lineage.of("email"), source("customers", "contact.email"));
    }
}

#[test]
fn replace_root_with_an_expression_gives_unknown() {
    for stage in [
        json!({"$replaceRoot": {"newRoot": {"$mergeObjects": ["$contact", {"id": "$_id"}]}}}),
        json!({"$replaceWith": {"$mergeObjects": ["$contact", "$address"]}}),
    ] {
        let lineage = aggregate(json!([stage]));

        assert_eq!(lineage.of("email"), FieldLineage::Unknown);
        assert_eq!(lineage.of("id"), FieldLineage::Unknown);
    }
}

#[test]
fn root_variable_gives_unknown() {
    let lineage = aggregate(json!([{"$project": {"whole": "$$ROOT", "same": "$$CURRENT.email"}}]));

    assert_eq!(lineage.of("whole"), FieldLineage::Unknown);
    assert_eq!(lineage.of("whole.email"), FieldLineage::Unknown);
    assert_eq!(lineage.of("same"), FieldLineage::Unknown);
}

#[test]
fn object_to_array_gives_unknown() {
    let lineage = aggregate(json!([{"$project": {"pairs": {"$objectToArray": "$contact"}}}]));

    assert_eq!(lineage.of("pairs"), FieldLineage::Unknown);
    assert_eq!(lineage.of("pairs.v"), FieldLineage::Unknown);
}

#[test]
fn facet_gives_unknown() {
    let lineage = aggregate(json!([{"$facet": {
        "byStatus": [{"$project": {"status": 1}}]
    }}]));

    assert_eq!(lineage.of("byStatus"), FieldLineage::Unknown);
    assert_eq!(lineage.of("byStatus.status"), FieldLineage::Unknown);
}

#[test]
fn computed_field_names_give_unknown() {
    let lineage = aggregate(json!([{"$project": {
        "named": {"$arrayToObject": [[["$email", "$name"]]]},
        "picked": {"$getField": {"field": "$label", "input": "$contact"}}
    }}]));

    assert_eq!(lineage.of("named"), FieldLineage::Unknown);
    assert_eq!(lineage.of("picked"), FieldLineage::Unknown);
}

#[test]
fn to_upper_of_a_clear_path_stays_clear() {
    let lineage = aggregate(json!([{"$project": {"loud": {"$toUpper": "$status"}}}]));

    assert_eq!(lineage.of("loud"), derived(&["status"], &["$toUpper"]));
    assert_eq!(
        clearance(&lineage.of("loud"), &clear_on(&["status"])),
        PROVEN_CLEAR
    );
}

#[test]
fn substring_of_a_path_not_clear_by_policy_is_tainted() {
    let lineage = aggregate(json!([{"$project": {
        "start": {"$substrCP": ["$email", 0, 3]}
    }}]));

    assert_eq!(lineage.of("start"), derived(&["email"], &["$substrCP"]));
    assert_eq!(
        clearance(&lineage.of("start"), &clear_on(&["status"])),
        TAINTED
    );
}

#[test]
fn operator_outside_the_allowlist_taints_even_clear_inputs() {
    let lineage = aggregate(json!([{"$project": {
        "start": {"$substrCP": ["$status", 0, 3]}
    }}]));

    assert_eq!(
        clearance(&lineage.of("start"), &clear_on(&["status"])),
        TAINTED
    );
}

#[test]
fn every_input_must_be_clear_by_policy() {
    let lineage = aggregate(json!([{"$project": {
        "label": {"$cond": [{"$eq": ["$status", "PAID"]}, "$status", "$email"]},
        "when": {"$dateToString": {"date": "$createdAt", "format": "%Y-%m"}}
    }}]));

    assert_eq!(
        clearance(&lineage.of("label"), &clear_on(&["status"])),
        TAINTED
    );
    assert_eq!(
        clearance(&lineage.of("label"), &clear_on(&["status", "email"])),
        PROVEN_CLEAR
    );
    assert_eq!(
        clearance(&lineage.of("when"), &clear_on(&["createdAt"])),
        PROVEN_CLEAR
    );
}

#[test]
fn an_input_cleared_by_policy_with_a_personal_field_below_stays_tainted() {
    let lineage = aggregate(json!([{"$project": {"copy": {"$ifNull": ["$contact", null]}}}]));
    let policy = policy(&[
        ("contact", PolicyDecision::Clear),
        ("contact.email", PolicyDecision::Personal("email".into())),
    ]);

    assert_eq!(clearance(&lineage.of("copy"), &policy), TAINTED);
}

#[test]
fn derived_inputs_keep_their_taint_through_later_stages() {
    let lineage = aggregate(json!([
        {"$set": {"start": {"$substrCP": ["$email", 0, 3]}}},
        {"$project": {"loud": {"$toUpper": "$start"}}}
    ]));

    assert_eq!(
        lineage.of("loud"),
        derived(&["email"], &["$substrCP", "$toUpper"])
    );
    assert_eq!(
        clearance(&lineage.of("loud"), &clear_on(&["email"])),
        TAINTED
    );
}

#[test]
fn user_variables_give_unknown_and_system_constants_are_clear() {
    let lineage = aggregate(json!([{"$project": {
        "tier": "$$tier",
        "at": "$$NOW"
    }}]));

    assert_eq!(lineage.of("tier"), FieldLineage::Unknown);
    assert_eq!(
        clearance(&lineage.of("at"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn count_and_sort_by_count_are_clear_counts() {
    let counted = aggregate(json!([{"$count": "n"}]));
    let sorted = aggregate(json!([{"$sortByCount": "$status"}]));

    assert_eq!(
        clearance(&counted.of("n"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
    assert_eq!(counted.of("email"), FieldLineage::Unknown);
    assert_eq!(sorted.of("_id"), source("customers", "status"));
    assert_eq!(
        clearance(&sorted.of("count"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn unwind_keeps_paths_and_its_index_is_clear() {
    let lineage =
        aggregate(json!([{"$unwind": {"path": "$orders", "includeArrayIndex": "position"}}]));

    assert_eq!(
        lineage.of("orders.total"),
        source("customers", "orders.total")
    );
    assert_eq!(
        clearance(&lineage.of("position"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn bucket_outputs_are_judged_like_group_accumulators() {
    let lineage = aggregate(json!([{"$bucket": {
        "groupBy": "$birthYear",
        "boundaries": [1950, 1970, 1990],
        "default": "other",
        "output": {"names": {"$push": "$name"}, "n": {"$sum": 1}}
    }}]));
    let auto = aggregate(json!([{"$bucketAuto": {"groupBy": "$birthYear", "buckets": 3}}]));

    assert_eq!(
        clearance(&lineage.of("_id"), &ColumnPolicy::default()),
        TAINTED
    );
    assert_eq!(
        clearance(&lineage.of("_id"), &clear_on(&["birthYear"])),
        PROVEN_CLEAR
    );
    assert_eq!(lineage.of("names"), source("customers", "name"));
    assert_eq!(
        clearance(&lineage.of("n"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
    assert_eq!(
        clearance(&auto.of("_id.min"), &ColumnPolicy::default()),
        TAINTED
    );
    assert_eq!(
        clearance(&auto.of("count"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn window_outputs_are_judged_like_group_accumulators() {
    let lineage = aggregate(json!([{"$setWindowFields": {
        "partitionBy": "$status",
        "sortBy": {"createdAt": 1},
        "output": {
            "running": {"$sum": "$amount", "window": {"documents": ["unbounded", "current"]}},
            "previous": {"$shift": {"output": "$email", "by": -1}}
        }
    }}]));

    assert_eq!(
        clearance(&lineage.of("running"), &clear_on(&["amount"])),
        PROVEN_CLEAR
    );
    assert_eq!(
        clearance(&lineage.of("previous"), &clear_on(&["email"])),
        TAINTED
    );
    assert_eq!(lineage.of("email"), source("customers", "email"));
}

#[test]
fn stages_whose_outputs_are_not_followed_give_unknown() {
    for (stage, path) in [
        (json!({"$unionWith": {"coll": "users"}}), "email"),
        (
            json!({"$fill": {"output": {"phone": {"value": "none"}}}}),
            "phone",
        ),
        (
            json!({"$densify": {"field": "createdAt", "range": {"step": 1, "unit": "day", "bounds": "full"}}}),
            "createdAt",
        ),
    ] {
        let lineage = aggregate(json!([stage]));

        assert_eq!(lineage.of(path), FieldLineage::Unknown, "{stage}");
    }
}

#[test]
fn filtering_and_ordering_stages_keep_the_document() {
    let lineage = aggregate(json!([
        {"$match": {"status": "PAID"}},
        {"$sort": {"createdAt": -1}},
        {"$skip": 1},
        {"$limit": 5},
        {"$sample": {"size": 2}}
    ]));

    assert_eq!(
        lineage.of("contact.email"),
        source("customers", "contact.email")
    );
}

#[test]
fn find_without_projection_keeps_every_path_as_source() {
    let lineage = find(None);

    assert_eq!(
        lineage.of("contact.email"),
        source("customers", "contact.email")
    );
}

#[test]
fn find_projection_with_an_expression_is_judged_like_project() {
    let lineage = find(Some(json!({
        "who": {"$concat": ["$firstName", " ", "$lastName"]},
        "status": 1
    })));

    assert_eq!(
        clearance(&lineage.of("who"), &clear_on(&["status"])),
        TAINTED
    );
    assert_eq!(lineage.of("status"), source("customers", "status"));
    assert_eq!(lineage.of("email"), FieldLineage::Unknown);
}

#[test]
fn bound_variables_give_unknown() {
    let lineage = aggregate(json!([{"$project": {
        "mapped": {"$map": {"input": "$emails", "in": {"$toUpper": "$$this"}}},
        "bound": {"$let": {"vars": {"e": "$email"}, "in": {"$toLower": "$$e"}}}
    }}]));

    assert_eq!(lineage.of("mapped"), FieldLineage::Unknown);
    assert_eq!(lineage.of("bound"), FieldLineage::Unknown);
}

#[test]
fn extended_json_and_literal_values_are_constants() {
    let lineage = aggregate(json!([{"$project": {
        "since": {"$literal": "$email"},
        "at": {"$dateToString": {"date": {"$date": "2026-01-01T00:00:00Z"}, "format": "%Y"}}
    }}]));

    assert_eq!(lineage.of("since"), derived(&[], &[]));
    assert_eq!(lineage.of("at"), derived(&[], &["$dateToString"]));
}

#[test]
fn a_built_document_is_judged_by_the_fields_it_holds() {
    let lineage = aggregate(json!([{"$project": {
        "x": {"a": "$email", "b": "$phone"},
        "label": {"kind": "VIP"}
    }}]));

    assert_eq!(
        clearance(&lineage.of("x"), &ColumnPolicy::default()),
        TAINTED
    );
    assert_eq!(
        clearance(&lineage.of("label"), &ColumnPolicy::default()),
        PROVEN_CLEAR
    );
}

#[test]
fn a_compound_group_key_is_judged_by_the_fields_it_holds() {
    let lineage =
        aggregate(json!([{"$group": {"_id": {"city": "$contact.city", "mail": "$email"}}}]));

    assert_eq!(
        clearance(&lineage.of("_id"), &clear_on(&["contact.city"])),
        TAINTED
    );
    assert_eq!(
        clearance(&lineage.of("_id"), &clear_on(&["contact.city", "email"])),
        PROVEN_CLEAR
    );
}

#[test]
fn a_source_document_with_an_overridden_field_is_no_longer_its_source() {
    let overridden = aggregate(json!([{"$set": {"contact.alias": "$email"}}]));
    let kept = aggregate(json!([{"$project": {"contact": 1}}]));

    assert_eq!(
        clearance(&overridden.of("contact"), &clear_on(&["contact"])),
        TAINTED
    );
    assert_eq!(kept.of("contact"), source("customers", "contact"));
}

#[test]
fn extended_json_projection_flags_give_unknown() {
    for projection in [
        json!({"email": {"$numberInt": "1"}}),
        json!({"_id": 0, "email": {"$numberLong": "1"}}),
        json!({"email": {"$numberDouble": "0"}}),
        json!({"contact": {"email": {"$numberDecimal": "1"}}}),
    ] {
        let piped = aggregate(json!([{"$project": projection.clone()}]));
        let found = find(Some(projection.clone()));

        for lineage in [piped, found] {
            assert_eq!(lineage.of("email"), FieldLineage::Unknown, "{projection}");
            assert_eq!(lineage.of("name"), FieldLineage::Unknown, "{projection}");
            assert_eq!(
                lineage.of("contact.email"),
                FieldLineage::Unknown,
                "{projection}"
            );
        }
    }
}

#[test]
fn stage_arguments_the_lineage_cannot_read_give_unknown() {
    for (stage, path) in [
        (
            json!({"$lookup": {"pipeline": [{"$documents": [{"x": 1}]}], "as": "extra"}}),
            "extra.x",
        ),
        (
            json!({"$unwind": {"path": "$orders", "includeArrayIndex": "$position"}}),
            "orders.total",
        ),
        (
            json!({"$geoNear": {"near": [0, 0], "distanceField": "distance", "includeLocs": "where"}}),
            "distance",
        ),
        (
            json!({"$geoNear": {"near": [0, 0], "distanceField": "distance", "includeLocs": "where"}}),
            "where",
        ),
        (json!({"$project": {"_id": 0, "items.$": 1}}), "items.total"),
    ] {
        let lineage = aggregate(json!([stage]));
        let expected = if path == "items.total" {
            source("customers", "items.total")
        } else {
            FieldLineage::Unknown
        };

        assert_eq!(lineage.of(path), expected, "{stage}");
    }
}

#[test]
fn a_personal_entry_below_an_input_counts_only_for_its_own_collection() {
    let lineage = aggregate(json!([{"$project": {"copy": {"$ifNull": ["$contact", null]}}}]));
    let mut elsewhere = clear_on(&["contact"]);
    elsewhere.insert_field(
        MongoFieldPattern {
            database: Some("shop".into()),
            collection: Some("users".into()),
            path: "contact.email".into(),
        },
        PolicyDecision::Personal("email".into()),
    );
    let mut everywhere = clear_on(&["contact"]);
    everywhere.insert_field(
        MongoFieldPattern {
            database: None,
            collection: None,
            path: "contact.email".into(),
        },
        PolicyDecision::Personal("email".into()),
    );

    assert_eq!(clearance(&lineage.of("copy"), &elsewhere), PROVEN_CLEAR);
    assert_eq!(clearance(&lineage.of("copy"), &everywhere), TAINTED);
}

#[test]
fn a_field_below_a_derived_value_is_judged_on_the_matching_input_field() {
    let lineage = aggregate(json!([{"$project": {"copy": {"$ifNull": ["$contact", null]}}}]));

    assert_eq!(
        lineage.of("copy.email"),
        derived(&["contact.email"], &["$ifNull"])
    );
    assert_eq!(
        clearance(&lineage.of("copy.email"), &clear_on(&["contact"])),
        TAINTED
    );
    assert_eq!(
        clearance(
            &lineage.of("copy.email"),
            &clear_on(&["contact", "contact.email"])
        ),
        PROVEN_CLEAR
    );
}
