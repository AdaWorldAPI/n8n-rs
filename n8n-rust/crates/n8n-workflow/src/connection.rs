//! Connection types and graph utilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Connection type identifier.
pub type ConnectionType = String;

/// Standard connection types.
pub const CONNECTION_MAIN: &str = "main";
pub const CONNECTION_ERROR: &str = "error";
pub const CONNECTION_AI_TOOL: &str = "ai_tool";
pub const CONNECTION_AI_LANGUAGE_MODEL: &str = "ai_languageModel";
pub const CONNECTION_AI_MEMORY: &str = "ai_memory";
pub const CONNECTION_AI_OUTPUT_PARSER: &str = "ai_outputParser";

/// Single connection endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Connection {
    /// Destination node name.
    pub node: String,
    /// Connection type at destination.
    #[serde(rename = "type")]
    pub connection_type: ConnectionType,
    /// Input/output index at destination.
    pub index: usize,
}

impl Connection {
    pub fn new(node: impl Into<String>, connection_type: impl Into<String>, index: usize) -> Self {
        Self {
            node: node.into(),
            connection_type: connection_type.into(),
            index,
        }
    }

    pub fn main(node: impl Into<String>, index: usize) -> Self {
        Self::new(node, CONNECTION_MAIN, index)
    }
}

/// Connections for a single output type indexed by output index.
/// connections[outputIndex] = [connections...]
pub type ConnectionsByIndex = Vec<Vec<Connection>>;

/// All connections from a node indexed by connection type.
/// connections[connectionType][outputIndex] = [connections...]
pub type NodeConnections = HashMap<ConnectionType, ConnectionsByIndex>;

/// All workflow connections indexed by source node name.
/// connections[sourceNodeName][connectionType][outputIndex] = [connections...]
pub type WorkflowConnections = HashMap<String, NodeConnections>;

/// Connections indexed by destination node for reverse lookups.
pub type ConnectionsByDestination = HashMap<String, Vec<ConnectionSource>>;

/// Source information for a connection.
#[derive(Debug, Clone)]
pub struct ConnectionSource {
    pub source_node: String,
    pub connection_type: ConnectionType,
    pub source_index: usize,
    pub dest_index: usize,
}

/// Utility functions for working with connections.
pub mod graph {
    use super::*;
    use std::collections::{HashSet, VecDeque};

    /// Map connections by destination node for efficient parent lookups.
    pub fn map_connections_by_destination(
        connections: &WorkflowConnections,
    ) -> ConnectionsByDestination {
        let mut result: ConnectionsByDestination = HashMap::new();

        for (source_node, node_connections) in connections {
            for (connection_type, by_index) in node_connections {
                for (source_index, connections_at_index) in by_index.iter().enumerate() {
                    for conn in connections_at_index {
                        result
                            .entry(conn.node.clone())
                            .or_default()
                            .push(ConnectionSource {
                                source_node: source_node.clone(),
                                connection_type: connection_type.clone(),
                                source_index,
                                dest_index: conn.index,
                            });
                    }
                }
            }
        }

        result
    }

    /// Get child nodes (successors) of a node.
    pub fn get_child_nodes(
        connections: &WorkflowConnections,
        node_name: &str,
        connection_type: Option<&str>,
        depth: Option<usize>,
    ) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((node_name.to_string(), 0usize));
        visited.insert(node_name.to_string());

        while let Some((current_node, current_depth)) = queue.pop_front() {
            if let Some(max_depth) = depth {
                if current_depth >= max_depth {
                    continue;
                }
            }

            if let Some(node_conns) = connections.get(&current_node) {
                for (conn_type, by_index) in node_conns {
                    if let Some(filter_type) = connection_type {
                        if conn_type != filter_type {
                            continue;
                        }
                    }

                    for connections_at_index in by_index {
                        for conn in connections_at_index {
                            if visited.insert(conn.node.clone()) {
                                result.push(conn.node.clone());
                                queue.push_back((conn.node.clone(), current_depth + 1));
                            }
                        }
                    }
                }
            }
        }

        result
    }

    /// Get parent nodes (predecessors) of a node.
    /// Note: Requires inverted connections from `map_connections_by_destination`.
    pub fn get_parent_nodes(
        connections_by_dest: &ConnectionsByDestination,
        node_name: &str,
        connection_type: Option<&str>,
        depth: Option<usize>,
    ) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((node_name.to_string(), 0usize));
        visited.insert(node_name.to_string());

        while let Some((current_node, current_depth)) = queue.pop_front() {
            if let Some(max_depth) = depth {
                if current_depth >= max_depth {
                    continue;
                }
            }

            if let Some(sources) = connections_by_dest.get(&current_node) {
                for source in sources {
                    if let Some(filter_type) = connection_type {
                        if source.connection_type != filter_type {
                            continue;
                        }
                    }

                    if visited.insert(source.source_node.clone()) {
                        result.push(source.source_node.clone());
                        queue.push_back((source.source_node.clone(), current_depth + 1));
                    }
                }
            }
        }

        result
    }

    /// Get all connected nodes (both directions).
    pub fn get_connected_nodes(
        connections: &WorkflowConnections,
        connections_by_dest: &ConnectionsByDestination,
        node_name: &str,
    ) -> Vec<String> {
        let mut result = HashSet::new();

        for child in get_child_nodes(connections, node_name, None, None) {
            result.insert(child);
        }

        for parent in get_parent_nodes(connections_by_dest, node_name, None, None) {
            result.insert(parent);
        }

        result.into_iter().collect()
    }

    /// Topological sort of nodes for execution ordering.
    pub fn topological_sort(
        node_names: &[String],
        connections: &WorkflowConnections,
    ) -> Result<Vec<String>, super::super::WorkflowError> {
        let connections_by_dest = map_connections_by_destination(connections);
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let node_set: HashSet<_> = node_names.iter().cloned().collect();

        // Initialize in-degrees
        for name in node_names {
            in_degree.insert(name.clone(), 0);
        }

        // Count incoming edges
        for name in node_names {
            if let Some(sources) = connections_by_dest.get(name) {
                let count = sources
                    .iter()
                    .filter(|s| node_set.contains(&s.source_node))
                    .count();
                *in_degree.get_mut(name).unwrap() = count;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            if let Some(node_conns) = connections.get(&node) {
                for by_index in node_conns.values() {
                    for connections_at_index in by_index {
                        for conn in connections_at_index {
                            if let Some(degree) = in_degree.get_mut(&conn.node) {
                                *degree = degree.saturating_sub(1);
                                if *degree == 0 {
                                    queue.push_back(conn.node.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        if result.len() != node_names.len() {
            return Err(super::super::WorkflowError::InvalidWorkflow(
                "Workflow contains a cycle".to_string(),
            ));
        }

        Ok(result)
    }
}
