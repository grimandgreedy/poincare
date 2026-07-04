//! Registry of builtin names, grouped by the categories frozen in the V1 spec.
//!
//! Phase 3 only needs the *names* so the resolver can tell a typo from a
//! builtin call. The actual builtin implementations arrive in language Phase 5.

pub const MATH: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "exp", "log", "log2", "log10", "sqrt",
    "pow", "abs", "sign", "round", "trunc", "floor", "ceil", "min", "max", "pi", "e",
];

pub const LIST: &[&str] = &["len", "sum", "mean", "prod", "map", "filter"];

pub const GRAPH: &[&str] = &[
    "graph",
    "add_plot",
    "surface",
    "curve",
    "scatter",
    "vector_field",
    "volume",
    "isosurface",
];

pub const TABLE: &[&str] = &["column", "columns", "rows", "array2d"];

pub const ANALYSIS: &[&str] = &["gradient", "derivative", "fit"];

pub const ATTACHMENT: &[&str] = &["attachment", "bytes", "text", "csv", "csv_matrix"];

pub const OUTPUT: &[&str] = &["print", "emit"];

const GROUPS: &[&[&str]] = &[MATH, LIST, GRAPH, TABLE, ANALYSIS, ATTACHMENT, OUTPUT];

/// Whether `name` is a known builtin.
pub fn is_builtin(name: &str) -> bool {
    GROUPS.iter().any(|group| group.contains(&name))
}

/// Iterate over every builtin name.
pub fn all() -> impl Iterator<Item = &'static str> {
    GROUPS.iter().flat_map(|group| group.iter().copied())
}
