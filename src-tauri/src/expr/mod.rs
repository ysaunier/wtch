/// jq-like expression evaluator for wtch conditions.
///
/// Expressions are evaluated against a `serde_json::Value` and resolve to either
/// a JSON value or a boolean (for use in monitor condition checks).
///
/// # Supported syntax
/// - Field access: `.field`, `.nested.field`, `.array[0]`
/// - Comparisons: `>  <  >=  <=  ==  !=`
/// - Arithmetic:  `+  -  *  /`
/// - Logic:       `and  or  not`
/// - Literals:    `"string"`, `true`, `false`, integers, floats
/// - Grouping:    `(.cost + .tax) > 100`
pub mod eval;
pub mod parser;

pub use eval::{evaluate, evaluate_bool};
pub use parser::ExprError;
