//! Extension methods for n8n expressions.
//!
//! Provides methods like .toUpperCase(), .toLowerCase(), .trim(), etc.
//! that can be called on values in expressions.

use super::{ExpressionContext, ExpressionError, ExpressionResult};
use serde_json::Value;

/// Call a method on a value.
pub fn call_method(
    value: &Value,
    method: &str,
    args: &[Value],
) -> ExpressionResult<Value> {
    match value {
        Value::String(s) => call_string_method(s, method, args),
        Value::Array(arr) => call_array_method(arr, method, args),
        Value::Number(n) => call_number_method(n, method, args),
        Value::Object(obj) => call_object_method(obj, method, args),
        Value::Bool(b) => call_bool_method(*b, method, args),
        Value::Null => {
            // Most methods on null return null
            Ok(Value::Null)
        }
    }
}

/// Call a global function.
pub fn call_function(
    name: &str,
    args: &[Value],
    _context: &ExpressionContext,
) -> ExpressionResult<Value> {
    match name {
        // Type checking
        "isEmpty" => func_is_empty(args),
        "isNotEmpty" => func_is_not_empty(args),
        "isBlank" => func_is_blank(args),

        // Type conversion
        "String" => func_to_string(args),
        "Number" => func_to_number(args),
        "Boolean" => func_to_boolean(args),

        // JSON operations
        "JSON" => Err(ExpressionError::MethodNotFound(
            "Use JSON.parse() or JSON.stringify()".to_string(),
        )),

        // Math functions
        "Math" => Err(ExpressionError::MethodNotFound(
            "Use Math.abs(), Math.floor(), etc.".to_string(),
        )),

        // Date functions
        "Date" => func_date(args),
        "DateTime" => func_datetime(args),

        // Object functions
        "Object" => Err(ExpressionError::MethodNotFound(
            "Use Object.keys(), Object.values(), etc.".to_string(),
        )),

        // Array functions
        "Array" => func_array(args),

        _ => Err(ExpressionError::MethodNotFound(format!(
            "Unknown function: {}",
            name
        ))),
    }
}

// =============================================================================
// String methods
// =============================================================================

fn call_string_method(s: &str, method: &str, args: &[Value]) -> ExpressionResult<Value> {
    match method {
        // Case conversion
        "toUpperCase" | "toLocaleUpperCase" => Ok(Value::String(s.to_uppercase())),
        "toLowerCase" | "toLocaleLowerCase" => Ok(Value::String(s.to_lowercase())),

        // Trimming
        "trim" => Ok(Value::String(s.trim().to_string())),
        "trimStart" | "trimLeft" => Ok(Value::String(s.trim_start().to_string())),
        "trimEnd" | "trimRight" => Ok(Value::String(s.trim_end().to_string())),

        // Padding
        "padStart" => {
            let length = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let pad_char = args
                .get(1)
                .and_then(|v| v.as_str())
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            if s.len() >= length {
                Ok(Value::String(s.to_string()))
            } else {
                let padding: String = std::iter::repeat(pad_char).take(length - s.len()).collect();
                Ok(Value::String(format!("{}{}", padding, s)))
            }
        }
        "padEnd" => {
            let length = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let pad_char = args
                .get(1)
                .and_then(|v| v.as_str())
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            if s.len() >= length {
                Ok(Value::String(s.to_string()))
            } else {
                let padding: String = std::iter::repeat(pad_char).take(length - s.len()).collect();
                Ok(Value::String(format!("{}{}", s, padding)))
            }
        }

        // Substring operations
        "slice" => {
            let start = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
            let end = args.get(1).and_then(|v| v.as_i64());
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;

            let start = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start.min(len) as usize
            };
            let end = match end {
                Some(e) if e < 0 => (len + e).max(0) as usize,
                Some(e) => e.min(len) as usize,
                None => len as usize,
            };

            if start >= end {
                Ok(Value::String(String::new()))
            } else {
                Ok(Value::String(chars[start..end].iter().collect()))
            }
        }
        "substring" => {
            let start = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let end = args.get(1).and_then(|v| v.as_u64());
            let chars: Vec<char> = s.chars().collect();

            let start = start.min(chars.len());
            let end = end.map(|e| (e as usize).min(chars.len())).unwrap_or(chars.len());

            if start >= end {
                Ok(Value::String(chars[end..start].iter().collect()))
            } else {
                Ok(Value::String(chars[start..end].iter().collect()))
            }
        }
        "substr" => {
            let start = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
            let length = args.get(1).and_then(|v| v.as_u64());
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;

            let start = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start.min(len) as usize
            };
            let end = match length {
                Some(l) => (start + l as usize).min(chars.len()),
                None => chars.len(),
            };

            Ok(Value::String(chars[start..end].iter().collect()))
        }

        // Search operations
        "includes" => {
            let search = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::Bool(s.contains(search)))
        }
        "startsWith" => {
            let search = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::Bool(s.starts_with(search)))
        }
        "endsWith" => {
            let search = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::Bool(s.ends_with(search)))
        }
        "indexOf" => {
            let search = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let index = s.find(search).map(|i| i as i64).unwrap_or(-1);
            Ok(Value::Number(index.into()))
        }
        "lastIndexOf" => {
            let search = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let index = s.rfind(search).map(|i| i as i64).unwrap_or(-1);
            Ok(Value::Number(index.into()))
        }

        // Replace operations
        "replace" => {
            let pattern = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let replacement = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::String(s.replacen(pattern, replacement, 1)))
        }
        "replaceAll" => {
            let pattern = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let replacement = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::String(s.replace(pattern, replacement)))
        }

        // Split and join
        "split" => {
            let separator = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let parts: Vec<Value> = if separator.is_empty() {
                s.chars().map(|c| Value::String(c.to_string())).collect()
            } else {
                s.split(separator)
                    .map(|p| Value::String(p.to_string()))
                    .collect()
            };
            Ok(Value::Array(parts))
        }

        // Character access
        "charAt" => {
            let index = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let char = s.chars().nth(index).map(|c| c.to_string()).unwrap_or_default();
            Ok(Value::String(char))
        }
        "charCodeAt" => {
            let index = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let code = s.chars().nth(index).map(|c| c as u32).unwrap_or(0);
            Ok(Value::Number(code.into()))
        }

        // Repeat
        "repeat" => {
            let count = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Ok(Value::String(s.repeat(count)))
        }

        // Concatenation
        "concat" => {
            let mut result = s.to_string();
            for arg in args {
                result.push_str(&arg.as_str().unwrap_or_default().to_string());
            }
            Ok(Value::String(result))
        }

        // Length
        "length" => Ok(Value::Number(s.len().into())),

        // n8n-specific extensions
        "toDate" => {
            // Parse as ISO date
            Ok(Value::String(s.to_string()))
        }
        "toDateTime" => {
            // Parse as ISO datetime
            Ok(Value::String(s.to_string()))
        }
        "extractEmail" => {
            // Simple email extraction
            let email_regex = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                .map_err(|_| ExpressionError::EvaluationError("Invalid regex".to_string()))?;
            let email = email_regex.find(s).map(|m| m.as_str().to_string());
            Ok(email.map(Value::String).unwrap_or(Value::Null))
        }
        "extractUrl" => {
            // Simple URL extraction
            let url_regex = regex::Regex::new(r"https?://[^\s]+")
                .map_err(|_| ExpressionError::EvaluationError("Invalid regex".to_string()))?;
            let url = url_regex.find(s).map(|m| m.as_str().to_string());
            Ok(url.map(Value::String).unwrap_or(Value::Null))
        }
        "isEmail" => {
            let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
                .map_err(|_| ExpressionError::EvaluationError("Invalid regex".to_string()))?;
            Ok(Value::Bool(email_regex.is_match(s)))
        }
        "isUrl" => {
            Ok(Value::Bool(s.starts_with("http://") || s.starts_with("https://")))
        }
        "isEmpty" => Ok(Value::Bool(s.is_empty())),
        "isNotEmpty" => Ok(Value::Bool(!s.is_empty())),
        "isBlank" => Ok(Value::Bool(s.trim().is_empty())),
        "isAlpha" => Ok(Value::Bool(s.chars().all(|c| c.is_alphabetic()))),
        "isAlphanumeric" => Ok(Value::Bool(s.chars().all(|c| c.is_alphanumeric()))),
        "isNumeric" => Ok(Value::Bool(s.parse::<f64>().is_ok())),

        // Hash functions
        "hash" => {
            let algorithm = args.first().and_then(|v| v.as_str()).unwrap_or("md5");
            hash_string(s, algorithm)
        }

        // Encoding
        "base64Encode" => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            Ok(Value::String(STANDARD.encode(s)))
        }
        "base64Decode" => {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            match STANDARD.decode(s) {
                Ok(bytes) => Ok(Value::String(String::from_utf8_lossy(&bytes).to_string())),
                Err(_) => Ok(Value::Null),
            }
        }
        "urlEncode" => {
            Ok(Value::String(urlencoding::encode(s).to_string()))
        }
        "urlDecode" => {
            Ok(Value::String(
                urlencoding::decode(s).unwrap_or_else(|_| std::borrow::Cow::Borrowed(s)).to_string(),
            ))
        }

        _ => Err(ExpressionError::MethodNotFound(format!(
            "String has no method '{}'",
            method
        ))),
    }
}

fn hash_string(s: &str, algorithm: &str) -> ExpressionResult<Value> {
    use sha2::{Digest, Sha256, Sha512};

    let result = match algorithm.to_lowercase().as_str() {
        "md5" => {
            // md5 crate uses a different API
            let digest = md5::compute(s.as_bytes());
            format!("{:x}", digest)
        }
        "sha256" => {
            let mut hasher = Sha256::new();
            hasher.update(s.as_bytes());
            format!("{:x}", hasher.finalize())
        }
        "sha512" => {
            let mut hasher = Sha512::new();
            hasher.update(s.as_bytes());
            format!("{:x}", hasher.finalize())
        }
        _ => {
            return Err(ExpressionError::InvalidArgument(format!(
                "Unknown hash algorithm: {}",
                algorithm
            )))
        }
    };

    Ok(Value::String(result))
}

// =============================================================================
// Array methods
// =============================================================================

fn call_array_method(arr: &[Value], method: &str, args: &[Value]) -> ExpressionResult<Value> {
    match method {
        // Length
        "length" => Ok(Value::Number(arr.len().into())),

        // Access
        "first" => Ok(arr.first().cloned().unwrap_or(Value::Null)),
        "last" => Ok(arr.last().cloned().unwrap_or(Value::Null)),

        // Search
        "includes" => {
            let search = args.first().unwrap_or(&Value::Null);
            Ok(Value::Bool(arr.contains(search)))
        }
        "indexOf" => {
            let search = args.first().unwrap_or(&Value::Null);
            let index = arr.iter().position(|v| v == search).map(|i| i as i64).unwrap_or(-1);
            Ok(Value::Number(index.into()))
        }
        "lastIndexOf" => {
            let search = args.first().unwrap_or(&Value::Null);
            let index = arr.iter().rposition(|v| v == search).map(|i| i as i64).unwrap_or(-1);
            Ok(Value::Number(index.into()))
        }
        "find" => {
            // Simplified find - looks for matching object
            if let Some(criteria) = args.first() {
                for item in arr {
                    if item == criteria {
                        return Ok(item.clone());
                    }
                }
            }
            Ok(Value::Null)
        }

        // Manipulation
        "concat" => {
            let mut result = arr.to_vec();
            for arg in args {
                if let Value::Array(other) = arg {
                    result.extend(other.clone());
                } else {
                    result.push(arg.clone());
                }
            }
            Ok(Value::Array(result))
        }
        "slice" => {
            let start = args.first().and_then(|v| v.as_i64()).unwrap_or(0);
            let end = args.get(1).and_then(|v| v.as_i64());
            let len = arr.len() as i64;

            let start = if start < 0 {
                (len + start).max(0) as usize
            } else {
                start.min(len) as usize
            };
            let end = match end {
                Some(e) if e < 0 => (len + e).max(0) as usize,
                Some(e) => e.min(len) as usize,
                None => len as usize,
            };

            if start >= end {
                Ok(Value::Array(vec![]))
            } else {
                Ok(Value::Array(arr[start..end].to_vec()))
            }
        }
        "reverse" => {
            let mut result = arr.to_vec();
            result.reverse();
            Ok(Value::Array(result))
        }
        "sort" => {
            let mut result = arr.to_vec();
            result.sort_by(|a, b| {
                match (a, b) {
                    (Value::Number(na), Value::Number(nb)) => {
                        na.as_f64().unwrap_or(0.0).partial_cmp(&nb.as_f64().unwrap_or(0.0))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (Value::String(sa), Value::String(sb)) => sa.cmp(sb),
                    _ => std::cmp::Ordering::Equal,
                }
            });
            Ok(Value::Array(result))
        }
        "flat" => {
            let depth = args.first().and_then(|v| v.as_u64()).unwrap_or(1);
            Ok(Value::Array(flatten_array(arr, depth as usize)))
        }

        // Join
        "join" => {
            let separator = args.first().and_then(|v| v.as_str()).unwrap_or(",");
            let joined: String = arr
                .iter()
                .map(|v| value_to_string(v))
                .collect::<Vec<_>>()
                .join(separator);
            Ok(Value::String(joined))
        }

        // Unique
        "unique" => {
            let mut seen = std::collections::HashSet::new();
            let mut result = Vec::new();
            for item in arr {
                let key = serde_json::to_string(item).unwrap_or_default();
                if seen.insert(key) {
                    result.push(item.clone());
                }
            }
            Ok(Value::Array(result))
        }

        // Compact (remove nulls)
        "compact" => {
            let result: Vec<Value> = arr
                .iter()
                .filter(|v| !v.is_null())
                .cloned()
                .collect();
            Ok(Value::Array(result))
        }

        // n8n-specific
        "isEmpty" => Ok(Value::Bool(arr.is_empty())),
        "isNotEmpty" => Ok(Value::Bool(!arr.is_empty())),

        // Pluck (extract field from array of objects)
        "pluck" => {
            let field = args.first().and_then(|v| v.as_str()).unwrap_or("");
            let result: Vec<Value> = arr
                .iter()
                .filter_map(|v| {
                    if let Value::Object(obj) = v {
                        obj.get(field).cloned()
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::Array(result))
        }

        // Randomize
        "randomItem" => {
            if arr.is_empty() {
                Ok(Value::Null)
            } else {
                use rand::Rng;
                let index = rand::thread_rng().gen_range(0..arr.len());
                Ok(arr[index].clone())
            }
        }
        "shuffle" => {
            use rand::seq::SliceRandom;
            let mut result = arr.to_vec();
            result.shuffle(&mut rand::thread_rng());
            Ok(Value::Array(result))
        }

        _ => Err(ExpressionError::MethodNotFound(format!(
            "Array has no method '{}'",
            method
        ))),
    }
}

fn flatten_array(arr: &[Value], depth: usize) -> Vec<Value> {
    if depth == 0 {
        return arr.to_vec();
    }

    let mut result = Vec::new();
    for item in arr {
        if let Value::Array(inner) = item {
            result.extend(flatten_array(inner, depth - 1));
        } else {
            result.push(item.clone());
        }
    }
    result
}

// =============================================================================
// Number methods
// =============================================================================

fn call_number_method(n: &serde_json::Number, method: &str, args: &[Value]) -> ExpressionResult<Value> {
    let value = n.as_f64().unwrap_or(0.0);

    match method {
        "toFixed" => {
            let digits = args.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Ok(Value::String(format!("{:.1$}", value, digits)))
        }
        "toPrecision" => {
            let precision = args.first().and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            Ok(Value::String(format!("{:.1$e}", value, precision.saturating_sub(1))))
        }
        "toString" => Ok(Value::String(n.to_string())),

        // Math operations
        "abs" => Ok(make_number(value.abs())),
        "ceil" => Ok(make_number(value.ceil())),
        "floor" => Ok(make_number(value.floor())),
        "round" => {
            let decimals = args.first().and_then(|v| v.as_u64()).unwrap_or(0);
            let factor = 10_f64.powi(decimals as i32);
            Ok(make_number((value * factor).round() / factor))
        }

        // n8n-specific
        "isEven" => Ok(Value::Bool(value as i64 % 2 == 0)),
        "isOdd" => Ok(Value::Bool(value as i64 % 2 != 0)),
        "format" => {
            // Simple number formatting
            let formatted = format!("{:.2}", value);
            Ok(Value::String(formatted))
        }

        _ => Err(ExpressionError::MethodNotFound(format!(
            "Number has no method '{}'",
            method
        ))),
    }
}

fn make_number(n: f64) -> Value {
    if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        Value::Number((n as i64).into())
    } else {
        Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| 0.into()))
    }
}

// =============================================================================
// Object methods
// =============================================================================

fn call_object_method(obj: &serde_json::Map<String, Value>, method: &str, args: &[Value]) -> ExpressionResult<Value> {
    match method {
        "keys" => {
            let keys: Vec<Value> = obj.keys().map(|k| Value::String(k.clone())).collect();
            Ok(Value::Array(keys))
        }
        "values" => {
            let values: Vec<Value> = obj.values().cloned().collect();
            Ok(Value::Array(values))
        }
        "entries" => {
            let entries: Vec<Value> = obj
                .iter()
                .map(|(k, v)| Value::Array(vec![Value::String(k.clone()), v.clone()]))
                .collect();
            Ok(Value::Array(entries))
        }
        "hasOwnProperty" | "hasField" => {
            let key = args.first().and_then(|v| v.as_str()).unwrap_or("");
            Ok(Value::Bool(obj.contains_key(key)))
        }
        "isEmpty" => Ok(Value::Bool(obj.is_empty())),
        "isNotEmpty" => Ok(Value::Bool(!obj.is_empty())),

        // Merge
        "merge" => {
            let mut result = obj.clone();
            for arg in args {
                if let Value::Object(other) = arg {
                    for (k, v) in other {
                        result.insert(k.clone(), v.clone());
                    }
                }
            }
            Ok(Value::Object(result))
        }

        // Pick/omit fields
        "pick" => {
            let keys: std::collections::HashSet<&str> = args
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let result: serde_json::Map<String, Value> = obj
                .iter()
                .filter(|(k, _)| keys.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Ok(Value::Object(result))
        }
        "omit" => {
            let keys: std::collections::HashSet<&str> = args
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let result: serde_json::Map<String, Value> = obj
                .iter()
                .filter(|(k, _)| !keys.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Ok(Value::Object(result))
        }

        _ => Err(ExpressionError::MethodNotFound(format!(
            "Object has no method '{}'",
            method
        ))),
    }
}

// =============================================================================
// Boolean methods
// =============================================================================

fn call_bool_method(b: bool, method: &str, _args: &[Value]) -> ExpressionResult<Value> {
    match method {
        "toString" => Ok(Value::String(b.to_string())),
        _ => Err(ExpressionError::MethodNotFound(format!(
            "Boolean has no method '{}'",
            method
        ))),
    }
}

// =============================================================================
// Global functions
// =============================================================================

fn func_is_empty(args: &[Value]) -> ExpressionResult<Value> {
    let value = args.first().unwrap_or(&Value::Null);
    let is_empty = match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(arr) => arr.is_empty(),
        Value::Object(obj) => obj.is_empty(),
        _ => false,
    };
    Ok(Value::Bool(is_empty))
}

fn func_is_not_empty(args: &[Value]) -> ExpressionResult<Value> {
    let result = func_is_empty(args)?;
    Ok(Value::Bool(!result.as_bool().unwrap_or(true)))
}

fn func_is_blank(args: &[Value]) -> ExpressionResult<Value> {
    let value = args.first().unwrap_or(&Value::Null);
    let is_blank = match value {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(arr) => arr.is_empty(),
        Value::Object(obj) => obj.is_empty(),
        _ => false,
    };
    Ok(Value::Bool(is_blank))
}

fn func_to_string(args: &[Value]) -> ExpressionResult<Value> {
    let value = args.first().unwrap_or(&Value::Null);
    Ok(Value::String(value_to_string(value)))
}

fn func_to_number(args: &[Value]) -> ExpressionResult<Value> {
    let value = args.first().unwrap_or(&Value::Null);
    let number = match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse().unwrap_or(0.0),
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        _ => 0.0,
    };
    Ok(make_number(number))
}

fn func_to_boolean(args: &[Value]) -> ExpressionResult<Value> {
    let value = args.first().unwrap_or(&Value::Null);
    let boolean = match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty() && s != "false" && s != "0",
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(_) => true,
    };
    Ok(Value::Bool(boolean))
}

fn func_date(args: &[Value]) -> ExpressionResult<Value> {
    use chrono::{NaiveDate, Utc};

    if args.is_empty() {
        let now = Utc::now();
        return Ok(Value::String(now.format("%Y-%m-%d").to_string()));
    }

    let input = args.first().and_then(|v| v.as_str()).unwrap_or("");

    // Try to parse as ISO date
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(Value::String(date.format("%Y-%m-%d").to_string()));
    }

    Ok(Value::Null)
}

fn func_datetime(args: &[Value]) -> ExpressionResult<Value> {
    use chrono::{DateTime, Utc};

    if args.is_empty() {
        let now = Utc::now();
        return Ok(Value::String(now.to_rfc3339()));
    }

    let input = args.first().and_then(|v| v.as_str()).unwrap_or("");

    // Try to parse as ISO datetime
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(Value::String(dt.to_rfc3339()));
    }

    Ok(Value::Null)
}

fn func_array(args: &[Value]) -> ExpressionResult<Value> {
    Ok(Value::Array(args.to_vec()))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_upper() {
        let result = call_string_method("hello", "toUpperCase", &[]).unwrap();
        assert_eq!(result, Value::String("HELLO".to_string()));
    }

    #[test]
    fn test_string_trim() {
        let result = call_string_method("  hello  ", "trim", &[]).unwrap();
        assert_eq!(result, Value::String("hello".to_string()));
    }

    #[test]
    fn test_string_includes() {
        let result =
            call_string_method("hello world", "includes", &[Value::String("world".to_string())])
                .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn test_array_join() {
        let arr = vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
            Value::String("c".to_string()),
        ];
        let result = call_array_method(&arr, "join", &[Value::String("-".to_string())]).unwrap();
        assert_eq!(result, Value::String("a-b-c".to_string()));
    }

    #[test]
    fn test_array_first_last() {
        let arr = vec![Value::Number(1.into()), Value::Number(2.into()), Value::Number(3.into())];
        assert_eq!(call_array_method(&arr, "first", &[]).unwrap(), Value::Number(1.into()));
        assert_eq!(call_array_method(&arr, "last", &[]).unwrap(), Value::Number(3.into()));
    }
}
