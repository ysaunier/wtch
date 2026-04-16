/// Evaluates a parsed expression AST against a `serde_json::Value`.
use serde_json::Value;

use super::parser::{parse, BinOp, Expr, ExprError, Literal, PathStep, UnaryOp};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluates an expression string against `data` and returns the resulting JSON
/// value.
///
/// # Errors
/// Returns [`ExprError`] for syntax errors, missing fields, type errors, and
/// division by zero.
pub fn evaluate(expression: &str, data: &Value) -> Result<Value, ExprError> {
    let ast = parse(expression)?;
    eval_expr(&ast, data)
}

/// Evaluates an expression string and coerces the result to a boolean.
///
/// Truthiness rules:
/// - `null`       -> false
/// - `false`      -> false
/// - `0` / `0.0`  -> false
/// - `""`         -> false
/// - everything else -> true
///
/// # Errors
/// Returns [`ExprError`] for any evaluation failure.
pub fn evaluate_bool(expression: &str, data: &Value) -> Result<bool, ExprError> {
    let value = evaluate(expression, data)?;
    Ok(is_truthy(&value))
}

// ---------------------------------------------------------------------------
// Truthiness helper
// ---------------------------------------------------------------------------

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---------------------------------------------------------------------------
// Core evaluator
// ---------------------------------------------------------------------------

fn eval_expr(expr: &Expr, data: &Value) -> Result<Value, ExprError> {
    match expr {
        Expr::Field(steps) => eval_field(steps, data),
        Expr::Literal(lit) => Ok(literal_to_value(lit)),
        Expr::BinaryOp { op, left, right } => eval_binary(op, left, right, data),
        Expr::UnaryOp { op, operand } => eval_unary(op, operand, data),
    }
}

// ---------------------------------------------------------------------------
// Field access
// ---------------------------------------------------------------------------

fn eval_field(steps: &[PathStep], data: &Value) -> Result<Value, ExprError> {
    let mut current = data;
    // We need an owned Value for intermediate steps; track via reference into
    // the original tree where possible, falling back to an owned allocation.
    let mut owned: Option<Value> = None;

    for step in steps {
        let node = owned.as_ref().unwrap_or(current);
        match step {
            PathStep::Key(key) => match node {
                Value::Object(map) => match map.get(key) {
                    Some(v) => {
                        owned = Some(v.clone());
                    }
                    None => return Err(ExprError::MissingField(key.clone())),
                },
                _ => {
                    return Err(ExprError::TypeError(format!(
                        "expected object to access field .{key}, got {node}"
                    )))
                }
            },
            PathStep::Index(idx) => match node {
                Value::Array(arr) => match arr.get(*idx) {
                    Some(v) => {
                        owned = Some(v.clone());
                    }
                    None => return Err(ExprError::IndexOutOfBounds(*idx)),
                },
                _ => {
                    return Err(ExprError::TypeError(format!(
                        "expected array to index [{idx}], got {node}"
                    )))
                }
            },
        }
        // After first step, current is no longer the primary source.
        let _ = current;
        current = data; // reset; owned now holds the value
    }

    Ok(owned.unwrap_or_else(|| data.clone()))
}

// ---------------------------------------------------------------------------
// Literal conversion
// ---------------------------------------------------------------------------

fn literal_to_value(lit: &Literal) -> Value {
    match lit {
        Literal::Number(n) => {
            // Preserve integer representation when possible.
            if n.fract() == 0.0 && n.abs() < 1e15 {
                Value::Number(serde_json::Number::from(*n as i64))
            } else {
                serde_json::Number::from_f64(*n)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
        }
        Literal::Str(s) => Value::String(s.clone()),
        Literal::Bool(b) => Value::Bool(*b),
        Literal::Null => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Binary operations
// ---------------------------------------------------------------------------

fn eval_binary(
    op: &BinOp,
    left: &Expr,
    right: &Expr,
    data: &Value,
) -> Result<Value, ExprError> {
    // Short-circuit logical operators before evaluating both sides.
    match op {
        BinOp::And => {
            let lv = eval_expr(left, data)?;
            if !is_truthy(&lv) {
                return Ok(Value::Bool(false));
            }
            let rv = eval_expr(right, data)?;
            return Ok(Value::Bool(is_truthy(&rv)));
        }
        BinOp::Or => {
            let lv = eval_expr(left, data)?;
            if is_truthy(&lv) {
                return Ok(Value::Bool(true));
            }
            let rv = eval_expr(right, data)?;
            return Ok(Value::Bool(is_truthy(&rv)));
        }
        _ => {}
    }

    let lv = eval_expr(left, data)?;
    let rv = eval_expr(right, data)?;

    match op {
        BinOp::Add => apply_add(&lv, &rv),
        BinOp::Sub => apply_arithmetic(&lv, &rv, |a, b| a - b),
        BinOp::Mul => apply_arithmetic(&lv, &rv, |a, b| a * b),
        BinOp::Div => {
            let b = as_f64(&rv)?;
            if b == 0.0 {
                return Err(ExprError::DivisionByZero);
            }
            let a = as_f64(&lv)?;
            Ok(f64_to_value(a / b))
        }
        BinOp::Gt => Ok(Value::Bool(compare_values(&lv, &rv)? > 0)),
        BinOp::Lt => Ok(Value::Bool(compare_values(&lv, &rv)? < 0)),
        BinOp::Gte => Ok(Value::Bool(compare_values(&lv, &rv)? >= 0)),
        BinOp::Lte => Ok(Value::Bool(compare_values(&lv, &rv)? <= 0)),
        BinOp::Eq => Ok(Value::Bool(values_equal(&lv, &rv))),
        BinOp::Neq => Ok(Value::Bool(!values_equal(&lv, &rv))),
        // And / Or are handled above.
        BinOp::And | BinOp::Or => unreachable!(),
    }
}

/// Addition: numbers add numerically, strings concatenate.
fn apply_add(a: &Value, b: &Value) -> Result<Value, ExprError> {
    match (a, b) {
        (Value::String(s1), Value::String(s2)) => {
            Ok(Value::String(format!("{s1}{s2}")))
        }
        _ => apply_arithmetic(a, b, |x, y| x + y),
    }
}

fn apply_arithmetic<F>(a: &Value, b: &Value, f: F) -> Result<Value, ExprError>
where
    F: Fn(f64, f64) -> f64,
{
    Ok(f64_to_value(f(as_f64(a)?, as_f64(b)?)))
}

/// Returns an integer JSON number when the value has no fractional part,
/// otherwise a float.
fn f64_to_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        Value::Number(serde_json::Number::from(n as i64))
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn as_f64(v: &Value) -> Result<f64, ExprError> {
    v.as_f64().ok_or_else(|| {
        ExprError::TypeError(format!("expected a number, got {v}"))
    })
}

/// Compares two values numerically or lexicographically.
/// Returns negative / zero / positive analogously to `Ord::cmp`.
fn compare_values(a: &Value, b: &Value) -> Result<i32, ExprError> {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => {
            let fa = as_f64(a)?;
            let fb = as_f64(b)?;
            Ok(fa.partial_cmp(&fb).map(|o| o as i32).unwrap_or(0))
        }
        (Value::String(sa), Value::String(sb)) => {
            Ok(sa.cmp(sb) as i32)
        }
        _ => Err(ExprError::TypeError(format!(
            "cannot compare {a} and {b}"
        ))),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    // Use JSON equality; numbers are compared by their numeric value.
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => {
            a.as_f64() == b.as_f64()
        }
        _ => a == b,
    }
}

// ---------------------------------------------------------------------------
// Unary operations
// ---------------------------------------------------------------------------

fn eval_unary(op: &UnaryOp, operand: &Expr, data: &Value) -> Result<Value, ExprError> {
    match op {
        UnaryOp::Not => {
            let v = eval_expr(operand, data)?;
            Ok(Value::Bool(!is_truthy(&v)))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Helper that panics with the error message on Err.
    fn eval(expr: &str, data: Value) -> Value {
        evaluate(expr, &data).expect("evaluate should succeed")
    }

    fn eval_b(expr: &str, data: Value) -> bool {
        evaluate_bool(expr, &data).expect("evaluate_bool should succeed")
    }

    // -- Basic field access --------------------------------------------------

    #[test]
    fn field_number() {
        assert_eq!(eval(".cost", json!({"cost": 60})), json!(60));
    }

    #[test]
    fn field_string() {
        assert_eq!(
            eval(".status", json!({"status": "ok"})),
            json!("ok")
        );
    }

    #[test]
    fn field_nested() {
        assert_eq!(
            eval(".a.b", json!({"a": {"b": 42}})),
            json!(42)
        );
    }

    #[test]
    fn field_array_index() {
        assert_eq!(
            eval(".items[0]", json!({"items": [10, 20, 30]})),
            json!(10)
        );
    }

    #[test]
    fn field_array_index_then_subfield() {
        assert_eq!(
            eval(".items[0].name", json!({"items": [{"name": "test"}]})),
            json!("test")
        );
    }

    // -- Comparisons ---------------------------------------------------------

    #[test]
    fn gt_true() {
        assert!(eval_b(".cost > 50", json!({"cost": 60})));
    }

    #[test]
    fn gt_false() {
        assert!(!eval_b(".cost > 50", json!({"cost": 40})));
    }

    #[test]
    fn lt_true() {
        assert!(eval_b(".cost < 100", json!({"cost": 60})));
    }

    #[test]
    fn gte_equal() {
        assert!(eval_b(".cost >= 60", json!({"cost": 60})));
    }

    #[test]
    fn lte_equal() {
        assert!(eval_b(".cost <= 60", json!({"cost": 60})));
    }

    #[test]
    fn eq_numbers() {
        assert!(eval_b(".x == 5", json!({"x": 5})));
    }

    #[test]
    fn neq_numbers() {
        assert!(eval_b(".x != 3", json!({"x": 5})));
    }

    // -- String equality -----------------------------------------------------

    #[test]
    fn status_eq_ok_true() {
        assert!(eval_b(r#".status == "ok""#, json!({"status": "ok"})));
    }

    #[test]
    fn status_eq_ok_false() {
        assert!(!eval_b(r#".status == "ok""#, json!({"status": "error"})));
    }

    // -- Arithmetic ----------------------------------------------------------

    #[test]
    fn cost_gt_limit_fraction() {
        // .cost > .limit * 0.8  =>  45 > 40  =>  true
        assert!(eval_b(
            ".cost > .limit * 0.8",
            json!({"cost": 45, "limit": 50})
        ));
    }

    #[test]
    fn add_fields() {
        // .a + .b > 10  =>  12 > 10  =>  true
        assert!(eval_b(".a + .b > 10", json!({"a": 5, "b": 7})));
    }

    #[test]
    fn sub_fields() {
        assert_eq!(eval(".a - .b", json!({"a": 10, "b": 3})), json!(7));
    }

    #[test]
    fn mul_literal() {
        assert_eq!(eval(".x * 3", json!({"x": 4})), json!(12));
    }

    #[test]
    fn div_exact() {
        assert_eq!(eval(".x / 2", json!({"x": 10})), json!(5));
    }

    #[test]
    fn div_by_zero() {
        let result = evaluate(".x / 0", &json!({"x": 10}));
        assert_eq!(result, Err(ExprError::DivisionByZero));
    }

    // -- Logic ---------------------------------------------------------------

    #[test]
    fn and_both_true() {
        assert!(eval_b(
            "(.x > 1) and (.y < 10)",
            json!({"x": 5, "y": 3})
        ));
    }

    #[test]
    fn and_one_false() {
        assert!(!eval_b(
            "(.x > 1) and (.y < 10)",
            json!({"x": 5, "y": 15})
        ));
    }

    #[test]
    fn or_one_true() {
        assert!(eval_b(
            ".x > 100 or .y < 10",
            json!({"x": 5, "y": 3})
        ));
    }

    #[test]
    fn not_false_field() {
        // `not .enabled` with enabled=false => true
        assert!(eval_b("not .enabled", json!({"enabled": false})));
    }

    #[test]
    fn not_true_field() {
        assert!(!eval_b("not .enabled", json!({"enabled": true})));
    }

    // -- Bare boolean field --------------------------------------------------

    #[test]
    fn bare_bool_true() {
        assert!(eval_b(".has_error", json!({"has_error": true})));
    }

    #[test]
    fn bare_bool_false() {
        assert!(!eval_b(".has_error", json!({"has_error": false})));
    }

    // -- Truthiness edge cases -----------------------------------------------

    #[test]
    fn truthy_zero_is_false() {
        assert!(!eval_b(".x", json!({"x": 0})));
    }

    #[test]
    fn truthy_empty_string_is_false() {
        assert!(!eval_b(".s", json!({"s": ""})));
    }

    #[test]
    fn truthy_null_is_false() {
        assert!(!eval_b(".v", json!({"v": null})));
    }

    #[test]
    fn truthy_nonzero_is_true() {
        assert!(eval_b(".x", json!({"x": 1})));
    }

    // -- Error cases ---------------------------------------------------------

    #[test]
    fn missing_field_returns_error() {
        let result = evaluate(".missing", &json!({}));
        assert_eq!(result, Err(ExprError::MissingField("missing".into())));
    }

    #[test]
    fn index_out_of_bounds() {
        let result = evaluate(".arr[5]", &json!({"arr": [1, 2]}));
        assert_eq!(result, Err(ExprError::IndexOutOfBounds(5)));
    }

    #[test]
    fn invalid_syntax_returns_error() {
        let result = evaluate("..cost", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn type_error_compare_string_to_number() {
        let result = evaluate(r#".name > 5"#, &json!({"name": "alice"}));
        assert!(matches!(result, Err(ExprError::TypeError(_))));
    }

    // -- String concatenation ------------------------------------------------

    #[test]
    fn string_concat_via_add() {
        assert_eq!(
            eval(r#".a + .b"#, json!({"a": "foo", "b": "bar"})),
            json!("foobar")
        );
    }
}
