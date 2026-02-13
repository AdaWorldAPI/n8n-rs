//! Built-in variables for n8n expressions.
//!
//! Provides resolution for variables like $json, $input, $node, $execution, etc.

use super::{ExpressionContext, ExpressionError, ExpressionResult};
use chrono::{Datelike, Utc};
use serde_json::Value;

/// Resolve a variable by name.
pub fn resolve_variable(name: &str, context: &ExpressionContext) -> ExpressionResult<Value> {
    match name {
        // Current item data
        "json" => resolve_json(context),
        "binary" => resolve_binary(context),

        // Input data references
        "input" => resolve_input(context),

        // Other node access
        "node" => resolve_node(context),

        // Execution context
        "execution" => resolve_execution(context),
        "workflow" => resolve_workflow(context),
        "runIndex" => Ok(Value::Number(context.run_index.into())),
        "itemIndex" => Ok(Value::Number(context.item_index.into())),

        // Variables and environment
        "vars" => resolve_vars(context),
        "env" => resolve_env(context),

        // Date/time
        "now" => Ok(Value::String(Utc::now().to_rfc3339())),
        "today" => {
            let now = Utc::now();
            Ok(Value::String(format!(
                "{:04}-{:02}-{:02}",
                now.year(),
                now.month(),
                now.day()
            )))
        }

        // JMESPath function (handled as function, not variable)
        "jmespath" => Err(ExpressionError::UndefinedVariable(
            "$jmespath should be called as a function: $jmespath(data, 'expression')".to_string(),
        )),

        // Unknown variable
        _ => Err(ExpressionError::UndefinedVariable(format!("${}", name))),
    }
}

/// Resolve $json - current item's JSON data.
fn resolve_json(context: &ExpressionContext) -> ExpressionResult<Value> {
    let json_map: serde_json::Map<String, Value> = context
        .item
        .json
        .iter()
        .map(|(k, v)| (k.clone(), data_value_to_json(v)))
        .collect();
    Ok(Value::Object(json_map))
}

/// Resolve $binary - current item's binary data references.
fn resolve_binary(context: &ExpressionContext) -> ExpressionResult<Value> {
    match &context.item.binary {
        Some(binary_map) => {
            let result: serde_json::Map<String, Value> = binary_map
                .iter()
                .map(|(k, v)| {
                    let bin_obj = serde_json::json!({
                        "mimeType": v.mime_type,
                        "fileName": v.file_name,
                        "fileSize": v.file_size,
                        "fileExtension": v.file_extension,
                    });
                    (k.clone(), bin_obj)
                })
                .collect();
            Ok(Value::Object(result))
        }
        None => Ok(Value::Object(serde_json::Map::new())),
    }
}

/// Resolve $input - input data reference.
fn resolve_input(context: &ExpressionContext) -> ExpressionResult<Value> {
    // $input provides access to the first input connection
    // Returns an object with methods like first(), last(), all(), item(n)
    Ok(Value::Object(serde_json::Map::from_iter([
        (
            "_type".to_string(),
            Value::String("InputReference".to_string()),
        ),
        (
            "context".to_string(),
            Value::String(context.node_name.to_string()),
        ),
    ])))
}

/// Resolve $node - access to other nodes' data.
fn resolve_node(context: &ExpressionContext) -> ExpressionResult<Value> {
    // $node["NodeName"] provides access to another node's data
    // Returns a proxy object that can be indexed
    let nodes: serde_json::Map<String, Value> = context
        .node_data
        .iter()
        .map(|(name, data)| {
            let node_output = data
                .first()
                .map(|items| {
                    Value::Array(
                        items
                            .iter()
                            .map(|item| {
                                let json_map: serde_json::Map<String, Value> = item
                                    .json
                                    .iter()
                                    .map(|(k, v)| (k.clone(), data_value_to_json(v)))
                                    .collect();
                                Value::Object(json_map)
                            })
                            .collect(),
                    )
                })
                .unwrap_or(Value::Array(vec![]));

            (
                name.clone(),
                serde_json::json!({
                    "json": node_output,
                    "first": node_output.as_array().and_then(|a| a.first()).cloned().unwrap_or(Value::Null),
                    "last": node_output.as_array().and_then(|a| a.last()).cloned().unwrap_or(Value::Null),
                }),
            )
        })
        .collect();

    Ok(Value::Object(nodes))
}

/// Resolve $execution - execution metadata.
fn resolve_execution(context: &ExpressionContext) -> ExpressionResult<Value> {
    Ok(serde_json::json!({
        "id": context.execution_id,
        "mode": "manual",
        "resumeUrl": null,
        "resumeFormUrl": null,
    }))
}

/// Resolve $workflow - workflow metadata.
fn resolve_workflow(context: &ExpressionContext) -> ExpressionResult<Value> {
    Ok(serde_json::json!({
        "id": context.workflow_id,
        "name": context.workflow_name,
        "active": true,
    }))
}

/// Resolve $vars - workflow variables.
fn resolve_vars(context: &ExpressionContext) -> ExpressionResult<Value> {
    let vars: serde_json::Map<String, Value> = context
        .variables
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(Value::Object(vars))
}

/// Resolve $env - environment variables.
fn resolve_env(context: &ExpressionContext) -> ExpressionResult<Value> {
    let env: serde_json::Map<String, Value> = context
        .env
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    Ok(Value::Object(env))
}

/// Convert GenericValue to JSON Value.
fn data_value_to_json(value: &n8n_workflow::GenericValue) -> Value {
    match value {
        n8n_workflow::GenericValue::Null => Value::Null,
        n8n_workflow::GenericValue::Bool(b) => Value::Bool(*b),
        n8n_workflow::GenericValue::Integer(i) => Value::Number((*i).into()),
        n8n_workflow::GenericValue::Float(f) => {
            Value::Number(serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()))
        }
        n8n_workflow::GenericValue::String(s) => Value::String(s.clone()),
        n8n_workflow::GenericValue::Array(arr) => {
            Value::Array(arr.iter().map(data_value_to_json).collect())
        }
        n8n_workflow::GenericValue::Object(obj) => {
            let map: serde_json::Map<String, Value> =
                obj.iter().map(|(k, v)| (k.clone(), data_value_to_json(v))).collect();
            Value::Object(map)
        }
    }
}

/// Node data accessor for $input.
pub struct InputAccessor<'a> {
    context: &'a ExpressionContext<'a>,
}

impl<'a> InputAccessor<'a> {
    pub fn new(context: &'a ExpressionContext<'a>) -> Self {
        Self { context }
    }

    /// Get the first item from input.
    pub fn first(&self) -> ExpressionResult<Value> {
        resolve_json(self.context)
    }

    /// Get the last item from input.
    pub fn last(&self) -> ExpressionResult<Value> {
        // For single item context, first == last
        resolve_json(self.context)
    }

    /// Get all items from input.
    pub fn all(&self) -> ExpressionResult<Value> {
        Ok(Value::Array(vec![resolve_json(self.context)?]))
    }

    /// Get item at specific index.
    pub fn item(&self, index: usize) -> ExpressionResult<Value> {
        if index == self.context.item_index {
            resolve_json(self.context)
        } else {
            Ok(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use n8n_workflow::{GenericValue, NodeExecutionData};

    #[test]
    fn test_resolve_json() {
        let mut item = NodeExecutionData::default();
        item.json
            .insert("name".to_string(), GenericValue::String("test".to_string()));
        item.json.insert("value".to_string(), GenericValue::Integer(42));

        let context = ExpressionContext::minimal(&item);
        let result = resolve_variable("json", &context).unwrap();

        assert!(result.is_object());
        assert_eq!(result["name"], Value::String("test".to_string()));
        assert_eq!(result["value"], Value::Number(42.into()));
    }

    #[test]
    fn test_resolve_now() {
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);
        let result = resolve_variable("now", &context).unwrap();

        assert!(result.is_string());
        // Should be an ISO timestamp
        assert!(result.as_str().unwrap().contains("T"));
    }

    #[test]
    fn test_resolve_today() {
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);
        let result = resolve_variable("today", &context).unwrap();

        assert!(result.is_string());
        // Should be YYYY-MM-DD format
        let date = result.as_str().unwrap();
        assert!(date.len() == 10);
        assert!(date.contains("-"));
    }

    #[test]
    fn test_resolve_undefined() {
        let item = NodeExecutionData::default();
        let context = ExpressionContext::minimal(&item);
        let result = resolve_variable("undefined_var", &context);

        assert!(result.is_err());
    }
}
