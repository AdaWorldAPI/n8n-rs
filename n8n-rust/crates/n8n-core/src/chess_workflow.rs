//! Chess self-play workflow definition.
//!
//! Defines a complete n8n `Workflow` for a chess self-play game between two
//! AI crews (White and Black), each backed by a crewai-rust `ChessThinkTank`.
//!
//! ## Workflow topology
//!
//! ```text
//!   [GameTrigger]
//!        |
//!   [Position]  <-----+
//!     /     \         |
//! [WhiteCrew] [BlackCrew]
//!     \     /         |
//!    [ApplyMove]      |
//!        |            |
//!   [StoreToNeo4j]    |
//!        |            |
//!   (loop-back if  ---+
//!    game not over)
//! ```
//!
//! The loop-back edge (`StoreToNeo4j` -> `Position`) forms a cycle, which is
//! intentional for the game loop.  Because `Workflow::validate()` rejects
//! cycles via topological sort, we build the workflow with
//! `WorkflowBuilder::build_unchecked()` and provide a dedicated
//! `ChessSelfPlayWorkflow::validate()` that permits this single back-edge.

use n8n_workflow::{
    connection::CONNECTION_MAIN,
    Node, NodeParameterValue, Workflow, WorkflowBuilder, WorkflowSettings,
};

// ============================================================================
// Node Type Constants
// ============================================================================

/// Node type for the game trigger (starts a new game).
pub const NODE_TYPE_GAME_TRIGGER: &str = "chess.gameTrigger";

/// Node type for the position manager (maintains FEN board state).
pub const NODE_TYPE_POSITION: &str = "chess.position";

/// Node type for the White crew delegation (crewai-rust ChessThinkTank).
pub const NODE_TYPE_WHITE_CREW: &str = "crew.chessThinkTank";

/// Node type for the Black crew delegation (crewai-rust ChessThinkTank).
pub const NODE_TYPE_BLACK_CREW: &str = "crew.chessThinkTank";

/// Node type for applying a chosen move and updating the FEN.
pub const NODE_TYPE_APPLY_MOVE: &str = "chess.applyMove";

/// Node type for storing the game/decision to the neo4j-rs knowledge graph.
pub const NODE_TYPE_STORE_NEO4J: &str = "chess.storeNeo4j";

// ============================================================================
// Node Name Constants
// ============================================================================

/// Display name for the game trigger node.
pub const NODE_NAME_GAME_TRIGGER: &str = "Game Trigger";

/// Display name for the position manager node.
pub const NODE_NAME_POSITION: &str = "Position Manager";

/// Display name for the White crew node.
pub const NODE_NAME_WHITE_CREW: &str = "White Crew";

/// Display name for the Black crew node.
pub const NODE_NAME_BLACK_CREW: &str = "Black Crew";

/// Display name for the apply-move node.
pub const NODE_NAME_APPLY_MOVE: &str = "Apply Move";

/// Display name for the neo4j store node.
pub const NODE_NAME_STORE_NEO4J: &str = "Store to Neo4j";

// ============================================================================
// Connection Name Constants
// ============================================================================

/// Connection label: trigger to position.
pub const CONN_TRIGGER_TO_POSITION: &str = "trigger_to_position";

/// Connection label: position to white crew.
pub const CONN_POSITION_TO_WHITE: &str = "position_to_white";

/// Connection label: position to black crew.
pub const CONN_POSITION_TO_BLACK: &str = "position_to_black";

/// Connection label: white crew to apply-move.
pub const CONN_WHITE_TO_MOVE: &str = "white_to_move";

/// Connection label: black crew to apply-move.
pub const CONN_BLACK_TO_MOVE: &str = "black_to_move";

/// Connection label: apply-move to store.
pub const CONN_MOVE_TO_STORE: &str = "move_to_store";

/// Connection label: store loop-back to position (game continuation).
pub const CONN_STORE_TO_POSITION_LOOP: &str = "store_to_position_loop";

// ============================================================================
// FEN Constants
// ============================================================================

/// Standard starting FEN for a chess game.
pub const STARTING_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

// ============================================================================
// GameConfig
// ============================================================================

/// Configuration for a chess self-play game.
#[derive(Debug, Clone)]
pub struct GameConfig {
    /// Starting FEN position (defaults to standard starting position).
    pub starting_fen: String,

    /// Neo4j connection URI for the knowledge graph store.
    pub neo4j_uri: String,

    /// Maximum number of moves before declaring a draw.
    pub max_moves: u32,

    /// Time budget per crew deliberation in milliseconds.
    pub think_time_ms: u64,

    /// CrewAI endpoint for White crew delegation.
    pub white_crew_endpoint: String,

    /// CrewAI endpoint for Black crew delegation.
    pub black_crew_endpoint: String,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            starting_fen: STARTING_FEN.to_string(),
            neo4j_uri: "bolt://localhost:7687".to_string(),
            max_moves: 200,
            think_time_ms: 30_000,
            white_crew_endpoint: "http://localhost:8081".to_string(),
            black_crew_endpoint: "http://localhost:8082".to_string(),
        }
    }
}

// ============================================================================
// GameOutcome
// ============================================================================

/// Result of a completed chess game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameOutcome {
    /// White wins (e.g. by checkmate).
    WhiteWins,
    /// Black wins (e.g. by checkmate).
    BlackWins,
    /// Game drawn (stalemate, repetition, 50-move rule, insufficient material).
    Draw,
    /// Game still in progress.
    InProgress,
    /// Game was abandoned (max moves exceeded, timeout, error).
    Abandoned { reason: String },
}

impl std::fmt::Display for GameOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameOutcome::WhiteWins => write!(f, "1-0"),
            GameOutcome::BlackWins => write!(f, "0-1"),
            GameOutcome::Draw => write!(f, "1/2-1/2"),
            GameOutcome::InProgress => write!(f, "*"),
            GameOutcome::Abandoned { reason } => write!(f, "* ({})", reason),
        }
    }
}

// ============================================================================
// ChessSelfPlayWorkflow
// ============================================================================

/// A self-play chess workflow between two AI crews.
///
/// This struct wraps an n8n `Workflow` and provides chess-specific accessors
/// and metadata.  The inner workflow contains:
///
/// 1. **Game Trigger** -- entry point that emits the starting FEN.
/// 2. **Position Manager** -- receives the current FEN, determines side to move.
/// 3. **White Crew** -- delegates to crewai-rust `ChessThinkTank` for White.
/// 4. **Black Crew** -- delegates to crewai-rust `ChessThinkTank` for Black.
/// 5. **Apply Move** -- validates and applies the chosen move, produces a new FEN.
/// 6. **Store to Neo4j** -- persists the position, move, and decision trail to
///    the neo4j-rs knowledge graph.
///
/// A loop-back connection from **Store to Neo4j** back to **Position Manager**
/// keeps the game running until a terminal state is reached.
#[derive(Debug, Clone)]
pub struct ChessSelfPlayWorkflow {
    /// The underlying n8n workflow definition.
    pub workflow: Workflow,

    /// Game configuration used to build this workflow.
    pub config: GameConfig,
}

impl ChessSelfPlayWorkflow {
    /// Create a new chess self-play workflow with the given configuration.
    pub fn new(config: GameConfig) -> Self {
        let workflow = chess_self_play_workflow(&config);
        Self { workflow, config }
    }

    /// Create a new chess self-play workflow with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(GameConfig::default())
    }

    /// Get a reference to the inner n8n workflow.
    pub fn workflow(&self) -> &Workflow {
        &self.workflow
    }

    /// Get the game trigger node.
    pub fn trigger_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_GAME_TRIGGER)
    }

    /// Get the position manager node.
    pub fn position_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_POSITION)
    }

    /// Get the White crew node.
    pub fn white_crew_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_WHITE_CREW)
    }

    /// Get the Black crew node.
    pub fn black_crew_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_BLACK_CREW)
    }

    /// Get the apply-move node.
    pub fn apply_move_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_APPLY_MOVE)
    }

    /// Get the neo4j store node.
    pub fn store_node(&self) -> Option<&Node> {
        self.workflow.get_node(NODE_NAME_STORE_NEO4J)
    }

    /// Validate the chess workflow structure.
    ///
    /// Unlike `Workflow::validate()`, this method explicitly permits the
    /// loop-back cycle from the store node to the position node.  All other
    /// structural invariants are still enforced:
    /// - All six nodes must be present.
    /// - Node names must be unique.
    /// - All connections must reference existing nodes.
    pub fn validate(&self) -> Result<(), n8n_workflow::WorkflowError> {
        let w = &self.workflow;

        // Verify all required nodes exist.
        let required_nodes = [
            NODE_NAME_GAME_TRIGGER,
            NODE_NAME_POSITION,
            NODE_NAME_WHITE_CREW,
            NODE_NAME_BLACK_CREW,
            NODE_NAME_APPLY_MOVE,
            NODE_NAME_STORE_NEO4J,
        ];
        for name in &required_nodes {
            if w.get_node(name).is_none() {
                return Err(n8n_workflow::WorkflowError::NodeNotFound(
                    name.to_string(),
                ));
            }
        }

        // Verify node count and name uniqueness.
        if w.nodes.len() < required_nodes.len() {
            return Err(n8n_workflow::WorkflowError::InvalidWorkflow(
                "Chess workflow must contain at least 6 nodes".to_string(),
            ));
        }

        let mut names = std::collections::HashSet::new();
        for node in &w.nodes {
            if !names.insert(&node.name) {
                return Err(n8n_workflow::WorkflowError::InvalidWorkflow(format!(
                    "Duplicate node name: {}",
                    node.name
                )));
            }
        }

        // Verify all connection targets reference existing nodes.
        for (source, node_conns) in &w.connections {
            if w.get_node(source).is_none() {
                return Err(n8n_workflow::WorkflowError::NodeNotFound(source.clone()));
            }
            for by_index in node_conns.values() {
                for connections_at_index in by_index {
                    for conn in connections_at_index {
                        if w.get_node(&conn.node).is_none() {
                            return Err(n8n_workflow::WorkflowError::NodeNotFound(
                                conn.node.clone(),
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Return the list of node names in logical execution order.
    ///
    /// Because the workflow contains a cycle, a true topological sort is not
    /// possible.  This returns the linear ordering for one pass through the
    /// game loop.
    pub fn execution_order(&self) -> Vec<&str> {
        vec![
            NODE_NAME_GAME_TRIGGER,
            NODE_NAME_POSITION,
            NODE_NAME_WHITE_CREW,
            NODE_NAME_BLACK_CREW,
            NODE_NAME_APPLY_MOVE,
            NODE_NAME_STORE_NEO4J,
        ]
    }
}

// ============================================================================
// Workflow Builder Function
// ============================================================================

/// Create a complete n8n `Workflow` definition for chess self-play.
///
/// This function builds all six nodes with their parameters and wires them
/// together according to the chess self-play topology.  The resulting workflow
/// intentionally contains a cycle (store -> position) for the game loop.
///
/// # Node details
///
/// | Node            | Type                  | Role                                        |
/// |-----------------|-----------------------|---------------------------------------------|
/// | Game Trigger    | `chess.gameTrigger`    | Emits starting FEN and game metadata        |
/// | Position Manager| `chess.position`      | Tracks current FEN, determines side to move |
/// | White Crew      | `crew.chessThinkTank` | Delegates to crewai-rust for White's move   |
/// | Black Crew      | `crew.chessThinkTank` | Delegates to crewai-rust for Black's move   |
/// | Apply Move      | `chess.applyMove`     | Validates move, updates FEN, detects endgame|
/// | Store to Neo4j  | `chess.storeNeo4j`    | Persists game state to neo4j-rs KG          |
///
/// # Connections
///
/// ```text
/// GameTrigger --(0,0)--> Position
/// Position    --(0,0)--> WhiteCrew
/// Position    --(0,0)--> BlackCrew   (parallel deliberation)
/// WhiteCrew   --(0,0)--> ApplyMove
/// BlackCrew   --(0,1)--> ApplyMove   (second input: Black's candidate)
/// ApplyMove   --(0,0)--> StoreToNeo4j
/// StoreToNeo4j--(0,0)--> Position    (loop-back)
/// ```
pub fn chess_self_play_workflow(config: &GameConfig) -> Workflow {
    // -- 1. Game Trigger node ---------------------------------------------------
    let mut trigger = Node::new(NODE_NAME_GAME_TRIGGER, NODE_TYPE_GAME_TRIGGER);
    trigger.position = [100.0, 300.0];
    trigger.set_parameter(
        "startingFen",
        NodeParameterValue::String(config.starting_fen.clone()),
    );
    trigger.set_parameter(
        "maxMoves",
        NodeParameterValue::Number(config.max_moves as f64),
    );
    trigger.notes = Some("Entry point: starts a new chess self-play game".to_string());

    // -- 2. Position Manager node -----------------------------------------------
    let mut position = Node::new(NODE_NAME_POSITION, NODE_TYPE_POSITION);
    position.position = [400.0, 300.0];
    position.set_parameter(
        "fen",
        NodeParameterValue::String(config.starting_fen.clone()),
    );
    position.set_parameter(
        "trackHistory",
        NodeParameterValue::Boolean(true),
    );
    position.notes = Some(
        "Manages board state (FEN). Receives initial FEN from trigger or \
         updated FEN from the loop-back after each move."
            .to_string(),
    );

    // -- 3. White Crew node -----------------------------------------------------
    let mut white_crew = Node::new(NODE_NAME_WHITE_CREW, NODE_TYPE_WHITE_CREW);
    white_crew.position = [700.0, 150.0];
    white_crew.set_parameter(
        "side",
        NodeParameterValue::String("white".to_string()),
    );
    white_crew.set_parameter(
        "role",
        NodeParameterValue::String("ChessThinkTank".to_string()),
    );
    white_crew.set_parameter(
        "endpoint",
        NodeParameterValue::String(config.white_crew_endpoint.clone()),
    );
    white_crew.set_parameter(
        "thinkTimeMs",
        NodeParameterValue::Number(config.think_time_ms as f64),
    );
    white_crew.notes = Some(
        "Delegates to crewai-rust ChessThinkTank crew for White. \
         Agents: Strategist, Tactician, Endgame Specialist."
            .to_string(),
    );

    // -- 4. Black Crew node -----------------------------------------------------
    let mut black_crew = Node::new(NODE_NAME_BLACK_CREW, NODE_TYPE_BLACK_CREW);
    black_crew.position = [700.0, 450.0];
    black_crew.set_parameter(
        "side",
        NodeParameterValue::String("black".to_string()),
    );
    black_crew.set_parameter(
        "role",
        NodeParameterValue::String("ChessThinkTank".to_string()),
    );
    black_crew.set_parameter(
        "endpoint",
        NodeParameterValue::String(config.black_crew_endpoint.clone()),
    );
    black_crew.set_parameter(
        "thinkTimeMs",
        NodeParameterValue::Number(config.think_time_ms as f64),
    );
    black_crew.notes = Some(
        "Delegates to crewai-rust ChessThinkTank crew for Black. \
         Agents: Strategist, Tactician, Endgame Specialist."
            .to_string(),
    );

    // -- 5. Apply Move node -----------------------------------------------------
    let mut apply_move = Node::new(NODE_NAME_APPLY_MOVE, NODE_TYPE_APPLY_MOVE);
    apply_move.position = [1000.0, 300.0];
    apply_move.set_parameter(
        "validateLegality",
        NodeParameterValue::Boolean(true),
    );
    apply_move.set_parameter(
        "detectEndgame",
        NodeParameterValue::Boolean(true),
    );
    apply_move.notes = Some(
        "Applies the chosen move to the board, updates the FEN, \
         and detects terminal conditions (checkmate, stalemate, draw)."
            .to_string(),
    );

    // -- 6. Store to Neo4j node -------------------------------------------------
    let mut store = Node::new(NODE_NAME_STORE_NEO4J, NODE_TYPE_STORE_NEO4J);
    store.position = [1300.0, 300.0];
    store.set_parameter(
        "neo4jUri",
        NodeParameterValue::String(config.neo4j_uri.clone()),
    );
    store.set_parameter(
        "storeDecisionTrail",
        NodeParameterValue::Boolean(true),
    );
    store.set_parameter(
        "storeAlternatives",
        NodeParameterValue::Boolean(true),
    );
    store.notes = Some(
        "Saves the game state, move, decision reasoning, and alternatives \
         to the neo4j-rs knowledge graph. If the game is not over, its \
         output feeds back to the Position Manager for the next move."
            .to_string(),
    );

    // -- Build the workflow with connections -------------------------------------
    //
    // We use `build_unchecked` because the loop-back edge creates a cycle that
    // the standard topological-sort validator would reject.  The dedicated
    // `ChessSelfPlayWorkflow::validate()` handles cycle-aware validation.

    let mut workflow = WorkflowBuilder::new("Chess Self-Play")
        .description(
            "Self-play chess game between two AI crews (White & Black), each \
             backed by a crewai-rust ChessThinkTank. Moves are persisted to \
             the neo4j-rs knowledge graph with full decision trails.",
        )
        .active(true)
        .node(trigger)
        .node(position)
        .node(white_crew)
        .node(black_crew)
        .node(apply_move)
        .node(store)
        .build_unchecked();

    // Wire connections.
    //
    // GameTrigger -> Position (output 0 -> input 0)
    let _ = workflow.connect(NODE_NAME_GAME_TRIGGER, NODE_NAME_POSITION, 0, 0);

    // Position -> WhiteCrew (output 0 -> input 0)
    let _ = workflow.connect(NODE_NAME_POSITION, NODE_NAME_WHITE_CREW, 0, 0);

    // Position -> BlackCrew (output 0 -> input 0)
    let _ = workflow.connect(NODE_NAME_POSITION, NODE_NAME_BLACK_CREW, 0, 0);

    // WhiteCrew -> ApplyMove (output 0 -> input 0: White's chosen move)
    let _ = workflow.connect(NODE_NAME_WHITE_CREW, NODE_NAME_APPLY_MOVE, 0, 0);

    // BlackCrew -> ApplyMove (output 0 -> input 1: Black's chosen move)
    let _ = workflow.connect(NODE_NAME_BLACK_CREW, NODE_NAME_APPLY_MOVE, 0, 1);

    // ApplyMove -> StoreToNeo4j (output 0 -> input 0)
    let _ = workflow.connect(NODE_NAME_APPLY_MOVE, NODE_NAME_STORE_NEO4J, 0, 0);

    // StoreToNeo4j -> Position (output 0 -> input 0) -- LOOP-BACK
    // This connection creates the game loop.  When the game is over, the
    // store node should produce no output on this connection, terminating
    // the loop.
    let _ = workflow.connect(NODE_NAME_STORE_NEO4J, NODE_NAME_POSITION, 0, 0);

    // Apply workflow settings.
    workflow.settings = WorkflowSettings {
        timezone: Some("UTC".to_string()),
        execution_timeout: Some(3600), // 1 hour max for a full game
        save_data_error_execution: Some(n8n_workflow::SaveDataOption::All),
        save_data_success_execution: Some(n8n_workflow::SaveDataOption::All),
        save_execution_progress: Some(true),
        ..WorkflowSettings::default()
    };

    workflow
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_workflow_has_all_nodes() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        assert_eq!(chess.workflow.nodes.len(), 6);

        assert!(chess.trigger_node().is_some());
        assert!(chess.position_node().is_some());
        assert!(chess.white_crew_node().is_some());
        assert!(chess.black_crew_node().is_some());
        assert!(chess.apply_move_node().is_some());
        assert!(chess.store_node().is_some());
    }

    #[test]
    fn test_trigger_node_is_trigger_type() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        let trigger = chess.trigger_node().unwrap();
        assert_eq!(trigger.node_type, NODE_TYPE_GAME_TRIGGER);
    }

    #[test]
    fn test_crew_nodes_have_correct_sides() {
        let chess = ChessSelfPlayWorkflow::with_defaults();

        let white = chess.white_crew_node().unwrap();
        let black = chess.black_crew_node().unwrap();

        let white_side = white.get_parameter("side").unwrap();
        let black_side = black.get_parameter("side").unwrap();

        match white_side {
            NodeParameterValue::String(s) => assert_eq!(s, "white"),
            _ => panic!("Expected string parameter for white side"),
        }

        match black_side {
            NodeParameterValue::String(s) => assert_eq!(s, "black"),
            _ => panic!("Expected string parameter for black side"),
        }
    }

    #[test]
    fn test_workflow_connections_exist() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        let conns = &chess.workflow.connections;

        // GameTrigger should connect to Position
        assert!(conns.contains_key(NODE_NAME_GAME_TRIGGER));

        // Position should connect to both crews
        assert!(conns.contains_key(NODE_NAME_POSITION));

        // Both crews should connect to ApplyMove
        assert!(conns.contains_key(NODE_NAME_WHITE_CREW));
        assert!(conns.contains_key(NODE_NAME_BLACK_CREW));

        // ApplyMove should connect to StoreToNeo4j
        assert!(conns.contains_key(NODE_NAME_APPLY_MOVE));

        // StoreToNeo4j should loop back to Position
        assert!(conns.contains_key(NODE_NAME_STORE_NEO4J));
    }

    #[test]
    fn test_loop_back_connection() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        let conns = &chess.workflow.connections;

        // Verify the store node connects back to the position node
        let store_conns = conns.get(NODE_NAME_STORE_NEO4J).unwrap();
        let main_conns = store_conns.get(CONNECTION_MAIN).unwrap();
        let first_output = &main_conns[0];
        assert_eq!(first_output.len(), 1);
        assert_eq!(first_output[0].node, NODE_NAME_POSITION);
    }

    #[test]
    fn test_custom_validate_accepts_cyclic_workflow() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        // The custom validate should succeed (allows the game loop cycle)
        assert!(chess.validate().is_ok());
    }

    #[test]
    fn test_standard_validate_rejects_cycle() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        // The standard workflow validate should reject the cycle
        assert!(chess.workflow.validate().is_err());
    }

    #[test]
    fn test_workflow_metadata() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        assert_eq!(chess.workflow.name, "Chess Self-Play");
        assert!(chess.workflow.active);
        assert!(chess.workflow.description.is_some());
    }

    #[test]
    fn test_custom_config() {
        let config = GameConfig {
            starting_fen: "8/8/8/4k3/8/8/4K3/8 w - - 0 1".to_string(),
            neo4j_uri: "bolt://neo4j.example.com:7687".to_string(),
            max_moves: 50,
            think_time_ms: 10_000,
            white_crew_endpoint: "http://white:9090".to_string(),
            black_crew_endpoint: "http://black:9091".to_string(),
        };
        let chess = ChessSelfPlayWorkflow::new(config.clone());

        // Verify starting FEN is propagated
        let trigger = chess.trigger_node().unwrap();
        match trigger.get_parameter("startingFen").unwrap() {
            NodeParameterValue::String(s) => {
                assert_eq!(s, "8/8/8/4k3/8/8/4K3/8 w - - 0 1");
            }
            _ => panic!("Expected string parameter"),
        }

        // Verify neo4j URI is propagated
        let store = chess.store_node().unwrap();
        match store.get_parameter("neo4jUri").unwrap() {
            NodeParameterValue::String(s) => {
                assert_eq!(s, "bolt://neo4j.example.com:7687");
            }
            _ => panic!("Expected string parameter"),
        }

        // Verify max moves is propagated
        match trigger.get_parameter("maxMoves").unwrap() {
            NodeParameterValue::Number(n) => assert_eq!(*n, 50.0),
            _ => panic!("Expected number parameter"),
        }
    }

    #[test]
    fn test_execution_order() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        let order = chess.execution_order();
        assert_eq!(order.len(), 6);
        assert_eq!(order[0], NODE_NAME_GAME_TRIGGER);
        assert_eq!(order[1], NODE_NAME_POSITION);
        assert_eq!(order[5], NODE_NAME_STORE_NEO4J);
    }

    #[test]
    fn test_game_outcome_display() {
        assert_eq!(GameOutcome::WhiteWins.to_string(), "1-0");
        assert_eq!(GameOutcome::BlackWins.to_string(), "0-1");
        assert_eq!(GameOutcome::Draw.to_string(), "1/2-1/2");
        assert_eq!(GameOutcome::InProgress.to_string(), "*");
        assert_eq!(
            GameOutcome::Abandoned {
                reason: "timeout".to_string()
            }
            .to_string(),
            "* (timeout)"
        );
    }

    #[test]
    fn test_workflow_serialization() {
        let chess = ChessSelfPlayWorkflow::with_defaults();
        // Should be serializable to JSON (important for persistence)
        let json = serde_json::to_string_pretty(&chess.workflow).unwrap();
        assert!(json.contains("Chess Self-Play"));
        assert!(json.contains(NODE_TYPE_GAME_TRIGGER));
        assert!(json.contains(NODE_TYPE_WHITE_CREW));

        // Should round-trip
        let back: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "Chess Self-Play");
        assert_eq!(back.nodes.len(), 6);
    }
}
