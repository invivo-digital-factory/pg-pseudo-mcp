pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_PIPELINE_STAGES: usize = 50;
pub const MAX_NESTING_DEPTH: usize = 32;
pub const MAX_ARRAY_ELEMENTS: usize = 1_000;
pub const MAX_ECHOED_NAME_CHARS: usize = 64;

pub const QUERY_COMMENT: &str = "scarif";

pub const STAGES: &[&str] = &[
    "$match",
    "$project",
    "$addFields",
    "$set",
    "$unset",
    "$group",
    "$sort",
    "$limit",
    "$skip",
    "$count",
    "$unwind",
    "$lookup",
    "$graphLookup",
    "$unionWith",
    "$facet",
    "$bucket",
    "$bucketAuto",
    "$sortByCount",
    "$replaceRoot",
    "$replaceWith",
    "$sample",
    "$densify",
    "$fill",
    "$setWindowFields",
    "$geoNear",
    "$documents",
];

pub const FIRST_STAGE_ONLY: &[&str] = &["$geoNear"];

pub const TOP_LEVEL_QUERY_OPERATORS: &[&str] =
    &["$and", "$or", "$nor", "$expr", "$text", "$comment"];

pub const FIELD_QUERY_OPERATORS: &[&str] = &[
    "$eq",
    "$ne",
    "$gt",
    "$gte",
    "$lt",
    "$lte",
    "$in",
    "$nin",
    "$not",
    "$exists",
    "$type",
    "$regex",
    "$options",
    "$mod",
    "$all",
    "$elemMatch",
    "$size",
    "$geoWithin",
    "$geoIntersects",
    "$near",
    "$nearSphere",
    "$maxDistance",
    "$minDistance",
];

pub const GEOMETRY_OPERATORS: &[&str] = &[
    "$geometry",
    "$box",
    "$center",
    "$centerSphere",
    "$polygon",
    "$maxDistance",
    "$minDistance",
];

pub const TEXT_OPERATORS: &[&str] = &[
    "$search",
    "$language",
    "$caseSensitive",
    "$diacriticSensitive",
];

pub const EXPRESSION_OPERATORS: &[&str] = &[
    "$abs",
    "$add",
    "$ceil",
    "$divide",
    "$exp",
    "$floor",
    "$ln",
    "$log",
    "$log10",
    "$mod",
    "$multiply",
    "$pow",
    "$round",
    "$sqrt",
    "$subtract",
    "$trunc",
    "$cmp",
    "$eq",
    "$gt",
    "$gte",
    "$lt",
    "$lte",
    "$ne",
    "$and",
    "$not",
    "$or",
    "$cond",
    "$ifNull",
    "$switch",
    "$concat",
    "$indexOfBytes",
    "$indexOfCP",
    "$ltrim",
    "$replaceOne",
    "$replaceAll",
    "$rtrim",
    "$split",
    "$strLenBytes",
    "$strLenCP",
    "$strcasecmp",
    "$substr",
    "$substrBytes",
    "$substrCP",
    "$toLower",
    "$toUpper",
    "$trim",
    "$regexFind",
    "$regexFindAll",
    "$regexMatch",
    "$dateAdd",
    "$dateDiff",
    "$dateFromParts",
    "$dateFromString",
    "$dateSubtract",
    "$dateToParts",
    "$dateToString",
    "$dateTrunc",
    "$dayOfMonth",
    "$dayOfWeek",
    "$dayOfYear",
    "$hour",
    "$isoDayOfWeek",
    "$isoWeek",
    "$isoWeekYear",
    "$millisecond",
    "$minute",
    "$month",
    "$second",
    "$week",
    "$year",
    "$convert",
    "$isNumber",
    "$toBool",
    "$toDate",
    "$toDecimal",
    "$toDouble",
    "$toInt",
    "$toLong",
    "$toObjectId",
    "$toString",
    "$toUUID",
    "$type",
    "$arrayElemAt",
    "$arrayToObject",
    "$concatArrays",
    "$filter",
    "$first",
    "$firstN",
    "$in",
    "$indexOfArray",
    "$isArray",
    "$last",
    "$lastN",
    "$map",
    "$maxN",
    "$minN",
    "$objectToArray",
    "$range",
    "$reduce",
    "$reverseArray",
    "$size",
    "$slice",
    "$sortArray",
    "$zip",
    "$allElementsTrue",
    "$anyElementTrue",
    "$setDifference",
    "$setEquals",
    "$setIntersection",
    "$setIsSubset",
    "$setUnion",
    "$mergeObjects",
    "$getField",
    "$setField",
    "$let",
    "$literal",
    "$rand",
    "$sum",
    "$avg",
    "$min",
    "$max",
    "$push",
    "$addToSet",
    "$count",
    "$top",
    "$bottom",
    "$topN",
    "$bottomN",
    "$median",
    "$percentile",
    "$stdDevPop",
    "$stdDevSamp",
    "$rank",
    "$denseRank",
    "$documentNumber",
    "$shift",
    "$derivative",
    "$integral",
    "$expMovingAvg",
    "$covariancePop",
    "$covarianceSamp",
    "$locf",
    "$linearFill",
];

pub const EXTENDED_JSON_TYPES: &[&str] = &[
    "$oid",
    "$numberInt",
    "$numberLong",
    "$numberDouble",
    "$numberDecimal",
    "$binary",
    "$uuid",
    "$date",
    "$timestamp",
    "$regularExpression",
    "$minKey",
    "$maxKey",
];

pub const EXTENDED_JSON_REFUSED: &[&str] = &["$code", "$symbol", "$dbPointer", "$undefined"];

pub const LOOKUP_ARGUMENTS: &[&str] = &[
    "from",
    "localField",
    "foreignField",
    "let",
    "pipeline",
    "as",
];

pub const GRAPH_LOOKUP_ARGUMENTS: &[&str] = &[
    "from",
    "startWith",
    "connectFromField",
    "connectToField",
    "as",
    "maxDepth",
    "depthField",
    "restrictSearchWithMatch",
];

pub const UNION_WITH_ARGUMENTS: &[&str] = &["coll", "pipeline"];

pub const UNWIND_ARGUMENTS: &[&str] = &["path", "includeArrayIndex", "preserveNullAndEmptyArrays"];

pub const BUCKET_ARGUMENTS: &[&str] = &["groupBy", "boundaries", "default", "output"];

pub const BUCKET_AUTO_ARGUMENTS: &[&str] = &["groupBy", "buckets", "output", "granularity"];

pub const REPLACE_ROOT_ARGUMENTS: &[&str] = &["newRoot"];

pub const SAMPLE_ARGUMENTS: &[&str] = &["size"];

pub const DENSIFY_ARGUMENTS: &[&str] = &["field", "partitionByFields", "range"];

pub const FILL_ARGUMENTS: &[&str] = &["partitionBy", "partitionByFields", "sortBy", "output"];

pub const FILL_OUTPUT_ARGUMENTS: &[&str] = &["value", "method"];

pub const SET_WINDOW_FIELDS_ARGUMENTS: &[&str] = &["partitionBy", "sortBy", "output"];

pub const WINDOW_ARGUMENT: &str = "window";

pub const GEO_NEAR_ARGUMENTS: &[&str] = &[
    "near",
    "distanceField",
    "spherical",
    "maxDistance",
    "minDistance",
    "query",
    "distanceMultiplier",
    "includeLocs",
    "key",
];

pub const COLLATION_ARGUMENTS: &[&str] = &[
    "locale",
    "caseLevel",
    "caseFirst",
    "strength",
    "numericOrdering",
    "alternate",
    "maxVariable",
    "backwards",
    "normalization",
];

pub const SYSTEM_DATABASES: &[&str] = &["admin", "local", "config"];

pub const SYSTEM_COLLECTION_PREFIX: &str = "system.";

pub const INVALID_DATABASE_CHARACTERS: &[char] = &[
    '/', '\\', '.', ' ', '"', '$', '*', '<', '>', ':', '|', '?', '\0',
];

pub const ALLOWLIST_HINT: &str = "only the read stages and operators listed by server_info are accepted; writes ($out, $merge), server-side JavaScript ($where, $function, $accumulator), admin and Atlas Search stages are refused";

pub const SYSTEM_NAMESPACE_HINT: &str = "use list_databases, list_tables or describe_table to explore structure; the admin, local and config databases and system collections are not readable";

pub const CROSS_DATABASE_HINT: &str = "$lookup, $graphLookup and $unionWith may only name a collection of the same database, as a string";

pub const NOT_TRANSLATED_POSITIONS: &str = "it is translated only in {field: value}, $eq, $ne, $in, $nin, $all and $elemMatch of a filter, and in $expr $eq, $ne or $in between a field path and a value";
