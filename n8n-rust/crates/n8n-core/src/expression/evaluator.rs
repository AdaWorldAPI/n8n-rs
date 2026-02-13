//! Expression evaluator for n8n expressions.

use super::parser::{BinaryOperator, Expr, Literal, TemplatePart, UnaryOperator};
use super::variables::resolve_variable;
use super::{ExpressionContext, ExpressionError, ExpressionResult};
use serde_json::Value;

/// Evaluator for n8n expressions.
pub struct ExpressionEvaluator {
    /// Whether to use strict mode (throw on undefined).
    pub strict: bool,
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpressionEvaluator {
    /// Create a new evaluator.
    pub fn new() -> Self {
        Self { strict: false }
    }

    /// Create a strict evaluator that throws on undefined.
    pub fn strict() -> Self {
        Self { strict: true }
    }

    /// Evaluate an expression AST.
    pub fn evaluate(
        &self,
        expr: &Expr,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        match expr {
            Expr::Literal(lit) => self.eval_literal(lit),
            Expr::Variable(name) => self.eval_variable(name, context),
            Expr::PropertyAccess { object, property } => {
                self.eval_property_access(object, property, context)
            }
            Expr::IndexAccess { object, index } => {
                self.eval_index_access(object, index, context)
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => self.eval_method_call(object, method, args, context),
            Expr::FunctionCall { name, args } => self.eval_function_call(name, args, context),
            Expr::BinaryOp { left, op, right } => self.eval_binary_op(left, *op, right, context),
            Expr::UnaryOp { op, operand } => self.eval_unary_op(*op, operand, context),
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
            } => self.eval_conditional(condition, then_expr, else_expr, context),
            Expr::Array(elements) => self.eval_array(elements, context),
            Expr::Object(pairs) => self.eval_object(pairs, context),
            Expr::Template(parts) => self.eval_template(parts, context),
        }
    }

    fn eval_literal(&self, lit: &Literal) -> ExpressionResult<Value> {
        Ok(match lit {
            Literal::Null => Value::Null,
            Literal::Boolean(b) => Value::Bool(*b),
            Literal::Number(n) => {
                if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    Value::Number((*n as i64).into())
                } else {
                    Value::Number(
                        serde_json::Number::from_f64(*n)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                }
            }
            Literal::String(s) => Value::String(s.clone()),
        })
    }

    fn eval_variable(
        &self,
        name: &str,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        resolve_variable(name, context)
    }

    fn eval_property_access(
        &self,
        object: &Expr,
        property: &str,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let obj = self.evaluate(object, context)?;

        match obj {
            Value::Object(map) => {
                if let Some(value) = map.get(property) {
                    Ok(value.clone())
                } else if self.strict {
                    Err(ExpressionError::PropertyNotFound(property.to_string()))
                } else {
                    Ok(Value::Null)
                }
            }
            Value::Null => {
                if self.strict {
                    Err(ExpressionError::PropertyNotFound(format!(
                        "Cannot access property '{}' of null",
                        property
                    )))
                } else {
                    Ok(Value::Null)
                }
            }
            _ => {
                if self.strict {
                    Err(ExpressionError::TypeError {
                        expected: "object".to_string(),
                        actual: value_type_name(&obj),
                    })
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }

    fn eval_index_access(
        &self,
        object: &Expr,
        index: &Expr,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let obj = self.evaluate(object, context)?;
        let idx = self.evaluate(index, context)?;

        match (&obj, &idx) {
            (Value::Array(arr), Value::Number(n)) => {
                let i = n.as_i64().unwrap_or(0) as usize;
                Ok(arr.get(i).cloned().unwrap_or(Value::Null))
            }
            (Value::Object(map), Value::String(key)) => {
                Ok(map.get(key).cloned().unwrap_or(Value::Null))
            }
            (Value::Object(map), Value::Number(n)) => {
                let key = n.to_string();
                Ok(map.get(&key).cloned().unwrap_or(Value::Null))
            }
            (Value::String(s), Value::Number(n)) => {
                let i = n.as_i64().unwrap_or(0) as usize;
                Ok(s.chars()
                    .nth(i)
                    .map(|c| Value::String(c.to_string()))
                    .unwrap_or(Value::Null))
            }
            (Value::Null, _) => Ok(Value::Null),
            _ => {
                if self.strict {
                    Err(ExpressionError::InvalidIndex(format!(
                        "Cannot index {} with {}",
                        value_type_name(&obj),
                        value_type_name(&idx)
                    )))
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }

    fn eval_method_call(
        &self,
        object: &Expr,
        method: &str,
        args: &[Expr],
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let obj = self.evaluate(object, context)?;
        let evaluated_args: Vec<Value> = args
            .iter()
            .map(|arg| self.evaluate(arg, context))
            .collect::<Result<_, _>>()?;

        super::extensions::call_method(&obj, method, &evaluated_args)
    }

    fn eval_function_call(
        &self,
        name: &str,
        args: &[Expr],
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let evaluated_args: Vec<Value> = args
            .iter()
            .map(|arg| self.evaluate(arg, context))
            .collect::<Result<_, _>>()?;

        super::extensions::call_function(name, &evaluated_args, context)
    }

    fn eval_binary_op(
        &self,
        left: &Expr,
        op: BinaryOperator,
        right: &Expr,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        // Short-circuit evaluation for && and ||
        if op == BinaryOperator::And {
            let left_val = self.evaluate(left, context)?;
            if !is_truthy(&left_val) {
                return Ok(left_val);
            }
            return self.evaluate(right, context);
        }
        if op == BinaryOperator::Or {
            let left_val = self.evaluate(left, context)?;
            if is_truthy(&left_val) {
                return Ok(left_val);
            }
            return self.evaluate(right, context);
        }
        if op == BinaryOperator::NullishCoalesce {
            let left_val = self.evaluate(left, context)?;
            if !left_val.is_null() {
                return Ok(left_val);
            }
            return self.evaluate(right, context);
        }

        let left_val = self.evaluate(left, context)?;
        let right_val = self.evaluate(right, context)?;

        match op {
            BinaryOperator::Add => self.eval_add(&left_val, &right_val),
            BinaryOperator::Sub => self.eval_sub(&left_val, &right_val),
            BinaryOperator::Mul => self.eval_mul(&left_val, &right_val),
            BinaryOperator::Div => self.eval_div(&left_val, &right_val),
            BinaryOperator::Mod => self.eval_mod(&left_val, &right_val),
            BinaryOperator::Eq => Ok(Value::Bool(values_equal(&left_val, &right_val))),
            BinaryOperator::Ne => Ok(Value::Bool(!values_equal(&left_val, &right_val))),
            BinaryOperator::Lt => self.eval_compare(&left_val, &right_val, |o| o.is_lt()),
            BinaryOperator::Le => self.eval_compare(&left_val, &right_val, |o| o.is_le()),
            BinaryOperator::Gt => self.eval_compare(&left_val, &right_val, |o| o.is_gt()),
            BinaryOperator::Ge => self.eval_compare(&left_val, &right_val, |o| o.is_ge()),
            BinaryOperator::And | BinaryOperator::Or | BinaryOperator::NullishCoalesce => {
                unreachable!("Handled above")
            }
        }
    }

    fn eval_add(&self, left: &Value, right: &Value) -> ExpressionResult<Value> {
        match (left, right) {
            (Value::Number(l), Value::Number(r)) => {
                let result = l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0);
                Ok(Value::Number(
                    serde_json::Number::from_f64(result)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
            (Value::String(l), Value::String(r)) => Ok(Value::String(format!("{}{}", l, r))),
            (Value::String(l), r) => Ok(Value::String(format!("{}{}", l, value_to_string(r)))),
            (l, Value::String(r)) => Ok(Value::String(format!("{}{}", value_to_string(l), r))),
            (Value::Array(l), Value::Array(r)) => {
                let mut result = l.clone();
                result.extend(r.clone());
                Ok(Value::Array(result))
            }
            _ => Ok(Value::Number(serde_json::Number::from(0))),
        }
    }

    fn eval_sub(&self, left: &Value, right: &Value) -> ExpressionResult<Value> {
        let l = value_to_number(left);
        let r = value_to_number(right);
        Ok(Value::Number(
            serde_json::Number::from_f64(l - r).unwrap_or_else(|| serde_json::Number::from(0)),
        ))
    }

    fn eval_mul(&self, left: &Value, right: &Value) -> ExpressionResult<Value> {
        let l = value_to_number(left);
        let r = value_to_number(right);
        Ok(Value::Number(
            serde_json::Number::from_f64(l * r).unwrap_or_else(|| serde_json::Number::from(0)),
        ))
    }

    fn eval_div(&self, left: &Value, right: &Value) -> ExpressionResult<Value> {
        let l = value_to_number(left);
        let r = value_to_number(right);
        if r == 0.0 {
            Ok(Value::Number(
                serde_json::Number::from_f64(f64::INFINITY)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        } else {
            Ok(Value::Number(
                serde_json::Number::from_f64(l / r).unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        }
    }

    fn eval_mod(&self, left: &Value, right: &Value) -> ExpressionResult<Value> {
        let l = value_to_number(left);
        let r = value_to_number(right);
        if r == 0.0 {
            Ok(Value::Number(
                serde_json::Number::from_f64(f64::NAN)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        } else {
            Ok(Value::Number(
                serde_json::Number::from_f64(l % r).unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        }
    }

    fn eval_compare<F>(&self, left: &Value, right: &Value, cmp: F) -> ExpressionResult<Value>
    where
        F: Fn(std::cmp::Ordering) -> bool,
    {
        let result = match (left, right) {
            (Value::Number(l), Value::Number(r)) => {
                let l = l.as_f64().unwrap_or(0.0);
                let r = r.as_f64().unwrap_or(0.0);
                cmp(l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal))
            }
            (Value::String(l), Value::String(r)) => cmp(l.cmp(r)),
            _ => false,
        };
        Ok(Value::Bool(result))
    }

    fn eval_unary_op(
        &self,
        op: UnaryOperator,
        operand: &Expr,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let val = self.evaluate(operand, context)?;

        match op {
            UnaryOperator::Not => Ok(Value::Bool(!is_truthy(&val))),
            UnaryOperator::Neg => {
                let n = value_to_number(&val);
                Ok(Value::Number(
                    serde_json::Number::from_f64(-n)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ))
            }
        }
    }

    fn eval_conditional(
        &self,
        condition: &Expr,
        then_expr: &Expr,
        else_expr: &Expr,
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let cond = self.evaluate(condition, context)?;
        if is_truthy(&cond) {
            self.evaluate(then_expr, context)
        } else {
            self.evaluate(else_expr, context)
        }
    }

    fn eval_array(
        &self,
        elements: &[Expr],
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let values: Vec<Value> = elements
            .iter()
            .map(|e| self.evaluate(e, context))
            .collect::<Result<_, _>>()?;
        Ok(Value::Array(values))
    }

    fn eval_object(
        &self,
        pairs: &[(String, Expr)],
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let mut map = serde_json::Map::new();
        for (key, value) in pairs {
            map.insert(key.clone(), self.evaluate(value, context)?);
        }
        Ok(Value::Object(map))
    }

    fn eval_template(
        &self,
        parts: &[TemplatePart],
        context: &ExpressionContext,
    ) -> ExpressionResult<Value> {
        let mut result = String::new();
        for part in parts {
            match part {
                TemplatePart::String(s) => result.push_str(s),
                TemplatePart::Expression(expr) => {
                    let value = self.evaluate(expr, context)?;
                    result.push_str(&value_to_string(&value));
                }
            }
        }
        Ok(Value::String(result))
    }
}

/// Check if a value is truthy.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(_) => true,
    }
}

/// Check if two values are equal.
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(l), Value::Bool(r)) => l == r,
        (Value::Number(l), Value::Number(r)) => {
            l.as_f64().unwrap_or(0.0) == r.as_f64().unwrap_or(0.0)
        }
        (Value::String(l), Value::String(r)) => l == r,
        (Value::Array(l), Value::Array(r)) => l == r,
        (Value::Object(l), Value::Object(r)) => l == r,
        // Loose equality for number/string
        (Value::Number(n), Value::String(s)) | (Value::String(s), Value::Number(n)) => {
            s.parse::<f64>().ok() == n.as_f64()
        }
        _ => false,
    }
}

/// Get type name for a value.
fn value_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

/// Convert a value to a string.
fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => serde_json::to_string(arr).unwrap_or_default(),
        Value::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
    }
}

/// Convert a value to a number.
fn value_to_number(value: &Value) -> f64 {
    match value {
        Value::Null => 0.0,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse().unwrap_or(0.0),
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

/// Resolve expressions in a node parameter value.
pub fn resolve_parameter(
    value: &Value,
    context: &ExpressionContext,
) -> ExpressionResult<Value> {
    let evaluator = ExpressionEvaluator::new();

    match value {
        Value::String(s) if s.contains("{{") => {
            let expr = super::parser::parse_template(s)?;
            evaluator.evaluate(&expr, context)
        }
        Value::Object(obj) => {
            let mut result = serde_json::Map::new();
            for (k, v) in obj {
                result.insert(k.clone(), resolve_parameter(v, context)?);
            }
            Ok(Value::Object(result))
        }
        Value::Array(arr) => {
            let result: Result<Vec<_>, _> = arr
                .iter()
                .map(|v| resolve_parameter(v, context))
                .collect();
            Ok(Value::Array(result?))
        }
        _ => Ok(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use n8n_workflow::NodeExecutionData;

    #[test]
    fn test_eval_literal() {
        let evaluator = ExpressionEvaluator::new();
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);

        let expr = super::super::parser::parse("42").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::Number(42.into()));

        let expr = super::super::parser::parse("\"hello\"").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::String("hello".to_string()));
    }

    #[test]
    fn test_eval_binary_op() {
        let evaluator = ExpressionEvaluator::new();
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);

        let expr = super::super::parser::parse("1 + 2").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::Number(3.into()));

        let expr = super::super::parser::parse("\"a\" + \"b\"").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::String("ab".to_string()));
    }

    #[test]
    fn test_eval_conditional() {
        let evaluator = ExpressionEvaluator::new();
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);

        let expr = super::super::parser::parse("true ? 1 : 2").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::Number(1.into()));

        let expr = super::super::parser::parse("false ? 1 : 2").unwrap();
        let result = evaluator.evaluate(&expr, &context).unwrap();
        assert_eq!(result, Value::Number(2.into()));
    }
}
