//! Built-in node type definitions.

use n8n_workflow::{
    NodeConnectionConfig, NodeCredentialDescription, NodeProperty, NodePropertyType,
    NodeTypeDescription, NodeVersion,
};

/// Get the description for a built-in node type.
pub fn get_node_type_description(node_type: &str) -> Option<NodeTypeDescription> {
    match node_type {
        "n8n-nodes-base.manualTrigger" => Some(manual_trigger_description()),
        "n8n-nodes-base.scheduleTrigger" => Some(schedule_trigger_description()),
        "n8n-nodes-base.webhook" => Some(webhook_trigger_description()),
        "n8n-nodes-base.set" => Some(set_description()),
        "n8n-nodes-base.if" => Some(if_description()),
        "n8n-nodes-base.merge" => Some(merge_description()),
        "n8n-nodes-base.code" => Some(code_description()),
        "n8n-nodes-base.httpRequest" => Some(http_request_description()),
        "n8n-nodes-base.noOp" => Some(no_op_description()),
        _ => None,
    }
}

fn manual_trigger_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.manualTrigger".to_string(),
        display_name: "Manual Trigger".to_string(),
        group: vec!["trigger".to_string()],
        description: "Triggers the workflow manually".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:play".to_string()),
        inputs: vec![],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![],
        credentials: None,
        trigger: true,
        polling: false,
    }
}

fn schedule_trigger_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.scheduleTrigger".to_string(),
        display_name: "Schedule Trigger".to_string(),
        group: vec!["trigger".to_string(), "schedule".to_string()],
        description: "Triggers the workflow on a time schedule".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:clock".to_string()),
        inputs: vec![],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![
            NodeProperty {
                name: "rule".to_string(),
                display_name: "Trigger Rule".to_string(),
                property_type: NodePropertyType::FixedCollection,
                default: None,
                description: Some("When the workflow should be triggered".to_string()),
                required: true,
                options: None,
                placeholder: None,
            },
            NodeProperty {
                name: "cronExpression".to_string(),
                display_name: "Cron Expression".to_string(),
                property_type: NodePropertyType::String,
                default: None,
                description: Some("Custom cron expression".to_string()),
                required: false,
                options: None,
                placeholder: Some("0 0 * * *".to_string()),
            },
        ],
        credentials: None,
        trigger: true,
        polling: true,
    }
}

fn webhook_trigger_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.webhook".to_string(),
        display_name: "Webhook".to_string(),
        group: vec!["trigger".to_string()],
        description: "Starts the workflow when a webhook is called".to_string(),
        version: NodeVersion::Multiple(vec![1, 2]),
        icon: Some("fa:bolt".to_string()),
        inputs: vec![],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![
            NodeProperty {
                name: "httpMethod".to_string(),
                display_name: "HTTP Method".to_string(),
                property_type: NodePropertyType::Options,
                default: None,
                description: Some("HTTP method to listen for".to_string()),
                required: false,
                options: None,
                placeholder: None,
            },
            NodeProperty {
                name: "path".to_string(),
                display_name: "Path".to_string(),
                property_type: NodePropertyType::String,
                default: None,
                description: Some("Webhook path".to_string()),
                required: true,
                options: None,
                placeholder: Some("/webhook-path".to_string()),
            },
            NodeProperty {
                name: "responseMode".to_string(),
                display_name: "Response Mode".to_string(),
                property_type: NodePropertyType::Options,
                default: None,
                description: Some("When to respond to the webhook".to_string()),
                required: false,
                options: None,
                placeholder: None,
            },
            NodeProperty {
                name: "responseData".to_string(),
                display_name: "Response Data".to_string(),
                property_type: NodePropertyType::Options,
                default: None,
                description: Some("What data to respond with".to_string()),
                required: false,
                options: None,
                placeholder: None,
            },
        ],
        credentials: None,
        trigger: true,
        polling: false,
    }
}

fn set_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.set".to_string(),
        display_name: "Set".to_string(),
        group: vec!["transform".to_string()],
        description: "Set values on items".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:pen".to_string()),
        inputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: true,
            max_connections: None,
        }],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![NodeProperty {
            name: "values".to_string(),
            display_name: "Values".to_string(),
            property_type: NodePropertyType::FixedCollection,
            default: None,
            description: Some("Values to set".to_string()),
            required: false,
            options: None,
            placeholder: None,
        }],
        credentials: None,
        trigger: false,
        polling: false,
    }
}

fn if_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.if".to_string(),
        display_name: "If".to_string(),
        group: vec!["flow".to_string()],
        description: "Route items based on conditions".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:code-branch".to_string()),
        inputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: true,
            max_connections: None,
        }],
        outputs: vec![
            NodeConnectionConfig {
                connection_type: "main".to_string(),
                display_name: Some("True".to_string()),
                required: false,
                max_connections: None,
            },
            NodeConnectionConfig {
                connection_type: "main".to_string(),
                display_name: Some("False".to_string()),
                required: false,
                max_connections: None,
            },
        ],
        default_input_name: None,
        default_output_name: None,
        properties: vec![NodeProperty {
            name: "conditions".to_string(),
            display_name: "Conditions".to_string(),
            property_type: NodePropertyType::Collection,
            default: None,
            description: Some("Conditions to check".to_string()),
            required: true,
            options: None,
            placeholder: None,
        }],
        credentials: None,
        trigger: false,
        polling: false,
    }
}

fn merge_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.merge".to_string(),
        display_name: "Merge".to_string(),
        group: vec!["flow".to_string()],
        description: "Merge multiple inputs into one".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:code-merge".to_string()),
        inputs: vec![
            NodeConnectionConfig {
                connection_type: "main".to_string(),
                display_name: Some("Input 1".to_string()),
                required: true,
                max_connections: None,
            },
            NodeConnectionConfig {
                connection_type: "main".to_string(),
                display_name: Some("Input 2".to_string()),
                required: true,
                max_connections: None,
            },
        ],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![NodeProperty {
            name: "mode".to_string(),
            display_name: "Mode".to_string(),
            property_type: NodePropertyType::Options,
            default: None,
            description: Some("How to merge the inputs".to_string()),
            required: false,
            options: None,
            placeholder: None,
        }],
        credentials: None,
        trigger: false,
        polling: false,
    }
}

fn code_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.code".to_string(),
        display_name: "Code".to_string(),
        group: vec!["transform".to_string()],
        description: "Execute custom code".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:code".to_string()),
        inputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: true,
            max_connections: None,
        }],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![NodeProperty {
            name: "code".to_string(),
            display_name: "Code".to_string(),
            property_type: NodePropertyType::String,
            default: None,
            description: Some("Code to execute".to_string()),
            required: true,
            options: None,
            placeholder: None,
        }],
        credentials: None,
        trigger: false,
        polling: false,
    }
}

fn http_request_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.httpRequest".to_string(),
        display_name: "HTTP Request".to_string(),
        group: vec!["output".to_string()],
        description: "Make HTTP requests".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:globe".to_string()),
        inputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: true,
            max_connections: None,
        }],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![
            NodeProperty {
                name: "url".to_string(),
                display_name: "URL".to_string(),
                property_type: NodePropertyType::String,
                default: None,
                description: Some("URL to request".to_string()),
                required: true,
                options: None,
                placeholder: Some("https://example.com".to_string()),
            },
            NodeProperty {
                name: "method".to_string(),
                display_name: "Method".to_string(),
                property_type: NodePropertyType::Options,
                default: None,
                description: Some("HTTP method".to_string()),
                required: false,
                options: None,
                placeholder: None,
            },
        ],
        credentials: Some(vec![NodeCredentialDescription {
            name: "httpBasicAuth".to_string(),
            required: false,
            display_options: None,
        }]),
        trigger: false,
        polling: false,
    }
}

fn no_op_description() -> NodeTypeDescription {
    NodeTypeDescription {
        name: "n8n-nodes-base.noOp".to_string(),
        display_name: "No Operation".to_string(),
        group: vec!["flow".to_string()],
        description: "Pass through without modification".to_string(),
        version: NodeVersion::Single(1),
        icon: Some("fa:arrow-right".to_string()),
        inputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: true,
            max_connections: None,
        }],
        outputs: vec![NodeConnectionConfig {
            connection_type: "main".to_string(),
            display_name: None,
            required: false,
            max_connections: None,
        }],
        default_input_name: None,
        default_output_name: None,
        properties: vec![],
        credentials: None,
        trigger: false,
        polling: false,
    }
}
