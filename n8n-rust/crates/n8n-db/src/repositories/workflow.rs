//! Workflow repository - CRUD operations for workflows.

use sqlx::PgPool;

use crate::entities::{
    InsertWorkflow, SharedWorkflow, UpdateWorkflow, WorkflowEntity, WorkflowHistory,
    WorkflowSharingRole, WorkflowTagMapping,
};
use crate::error::DbError;

/// Repository for workflow operations.
#[derive(Clone)]
pub struct WorkflowRepository {
    pool: PgPool,
}

impl WorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a workflow by ID.
    pub async fn find_by_id(&self, id: &str) -> Result<Option<WorkflowEntity>, DbError> {
        let workflow = sqlx::query_as::<_, WorkflowEntity>(
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            WHERE id = $1 AND is_archived = false
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(workflow)
    }

    /// Get a workflow by name.
    pub async fn find_by_name(&self, name: &str) -> Result<Option<WorkflowEntity>, DbError> {
        let workflow = sqlx::query_as::<_, WorkflowEntity>(
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            WHERE name = $1 AND is_archived = false
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(workflow)
    }

    /// List all workflows.
    pub async fn find_all(&self, include_archived: bool) -> Result<Vec<WorkflowEntity>, DbError> {
        let query = if include_archived {
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            ORDER BY updated_at DESC
            "#
        } else {
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            WHERE is_archived = false
            ORDER BY updated_at DESC
            "#
        };

        let workflows = sqlx::query_as::<_, WorkflowEntity>(query)
            .fetch_all(&self.pool)
            .await?;

        Ok(workflows)
    }

    /// List active workflows.
    pub async fn find_active(&self) -> Result<Vec<WorkflowEntity>, DbError> {
        let workflows = sqlx::query_as::<_, WorkflowEntity>(
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            WHERE active = true AND is_archived = false
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(workflows)
    }

    /// List workflows in a folder.
    pub async fn find_by_folder(&self, folder_id: &str) -> Result<Vec<WorkflowEntity>, DbError> {
        let workflows = sqlx::query_as::<_, WorkflowEntity>(
            r#"
            SELECT id, name, description, active, is_archived, nodes, connections,
                   settings, static_data, meta, pin_data, version_id, active_version_id,
                   version_counter, trigger_count, parent_folder_id, created_at, updated_at
            FROM workflow_entity
            WHERE parent_folder_id = $1 AND is_archived = false
            ORDER BY name ASC
            "#,
        )
        .bind(folder_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(workflows)
    }

    /// Create a new workflow.
    pub async fn create(&self, workflow: &InsertWorkflow) -> Result<WorkflowEntity, DbError> {
        let created = sqlx::query_as::<_, WorkflowEntity>(
            r#"
            INSERT INTO workflow_entity (
                id, name, description, nodes, connections, settings,
                static_data, meta, pin_data, version_id, parent_folder_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, name, description, active, is_archived, nodes, connections,
                      settings, static_data, meta, pin_data, version_id, active_version_id,
                      version_counter, trigger_count, parent_folder_id, created_at, updated_at
            "#,
        )
        .bind(&workflow.id)
        .bind(&workflow.name)
        .bind(&workflow.description)
        .bind(&workflow.nodes)
        .bind(&workflow.connections)
        .bind(&workflow.settings)
        .bind(&workflow.static_data)
        .bind(&workflow.meta)
        .bind(&workflow.pin_data)
        .bind(&workflow.version_id)
        .bind(&workflow.parent_folder_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Update a workflow.
    pub async fn update(&self, id: &str, update: &UpdateWorkflow) -> Result<WorkflowEntity, DbError> {
        // Build dynamic update query
        let mut set_clauses = Vec::new();
        let mut param_idx = 2; // $1 is id

        if update.name.is_some() {
            set_clauses.push(format!("name = ${}", param_idx));
            param_idx += 1;
        }
        if update.description.is_some() {
            set_clauses.push(format!("description = ${}", param_idx));
            param_idx += 1;
        }
        if update.active.is_some() {
            set_clauses.push(format!("active = ${}", param_idx));
            param_idx += 1;
        }
        if update.is_archived.is_some() {
            set_clauses.push(format!("is_archived = ${}", param_idx));
            param_idx += 1;
        }
        if update.nodes.is_some() {
            set_clauses.push(format!("nodes = ${}", param_idx));
            param_idx += 1;
        }
        if update.connections.is_some() {
            set_clauses.push(format!("connections = ${}", param_idx));
            param_idx += 1;
        }

        // Always increment version counter
        set_clauses.push("version_counter = version_counter + 1".to_string());
        set_clauses.push("updated_at = NOW()".to_string());

        if set_clauses.is_empty() {
            return self.find_by_id(id).await?.ok_or(DbError::NotFound);
        }

        let query = format!(
            r#"
            UPDATE workflow_entity
            SET {}
            WHERE id = $1
            RETURNING id, name, description, active, is_archived, nodes, connections,
                      settings, static_data, meta, pin_data, version_id, active_version_id,
                      version_counter, trigger_count, parent_folder_id, created_at, updated_at
            "#,
            set_clauses.join(", ")
        );

        let mut query = sqlx::query_as::<_, WorkflowEntity>(&query).bind(id);

        if let Some(ref name) = update.name {
            query = query.bind(name);
        }
        if let Some(ref desc) = update.description {
            query = query.bind(desc);
        }
        if let Some(active) = update.active {
            query = query.bind(active);
        }
        if let Some(archived) = update.is_archived {
            query = query.bind(archived);
        }
        if let Some(ref nodes) = update.nodes {
            query = query.bind(nodes);
        }
        if let Some(ref conns) = update.connections {
            query = query.bind(conns);
        }

        let updated = query.fetch_one(&self.pool).await?;
        Ok(updated)
    }

    /// Archive a workflow (soft delete).
    pub async fn archive(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            UPDATE workflow_entity
            SET is_archived = true, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Permanently delete a workflow.
    pub async fn delete(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query("DELETE FROM workflow_entity WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Activate a workflow.
    pub async fn activate(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            UPDATE workflow_entity
            SET active = true, updated_at = NOW()
            WHERE id = $1 AND is_archived = false
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Deactivate a workflow.
    pub async fn deactivate(&self, id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            r#"
            UPDATE workflow_entity
            SET active = false, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // Workflow History
    // =========================================================================

    /// Create a history entry for a workflow.
    pub async fn create_history(&self, history: &WorkflowHistory) -> Result<WorkflowHistory, DbError> {
        let nodes_json = serde_json::to_value(&history.nodes)?;

        let created = sqlx::query_as::<_, WorkflowHistory>(
            r#"
            INSERT INTO workflow_history (
                version_id, workflow_id, nodes, connections, authors,
                name, description, autosaved
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING version_id, workflow_id, nodes, connections, authors,
                      name, description, autosaved, created_at, updated_at
            "#,
        )
        .bind(&history.version_id)
        .bind(&history.workflow_id)
        .bind(&nodes_json)
        .bind(&history.connections)
        .bind(&history.authors)
        .bind(&history.name)
        .bind(&history.description)
        .bind(history.autosaved)
        .fetch_one(&self.pool)
        .await?;

        Ok(created)
    }

    /// Get history entries for a workflow.
    pub async fn get_history(&self, workflow_id: &str, limit: i64) -> Result<Vec<WorkflowHistory>, DbError> {
        let history = sqlx::query_as::<_, WorkflowHistory>(
            r#"
            SELECT version_id, workflow_id, nodes, connections, authors,
                   name, description, autosaved, created_at, updated_at
            FROM workflow_history
            WHERE workflow_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(workflow_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(history)
    }

    /// Get a specific history version.
    pub async fn get_history_version(&self, version_id: &str) -> Result<Option<WorkflowHistory>, DbError> {
        let history = sqlx::query_as::<_, WorkflowHistory>(
            r#"
            SELECT version_id, workflow_id, nodes, connections, authors,
                   name, description, autosaved, created_at, updated_at
            FROM workflow_history
            WHERE version_id = $1
            "#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(history)
    }

    // =========================================================================
    // Sharing
    // =========================================================================

    /// Share a workflow with a project.
    pub async fn share(
        &self,
        workflow_id: &str,
        project_id: &str,
        role: WorkflowSharingRole,
    ) -> Result<SharedWorkflow, DbError> {
        let shared = sqlx::query_as::<_, SharedWorkflow>(
            r#"
            INSERT INTO shared_workflow (workflow_id, project_id, role)
            VALUES ($1, $2, $3)
            ON CONFLICT (workflow_id, project_id) DO UPDATE SET role = $3
            RETURNING workflow_id, project_id, role, created_at, updated_at
            "#,
        )
        .bind(workflow_id)
        .bind(project_id)
        .bind(role.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(shared)
    }

    /// Unshare a workflow from a project.
    pub async fn unshare(&self, workflow_id: &str, project_id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            "DELETE FROM shared_workflow WHERE workflow_id = $1 AND project_id = $2",
        )
        .bind(workflow_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get sharing info for a workflow.
    pub async fn get_sharing(&self, workflow_id: &str) -> Result<Vec<SharedWorkflow>, DbError> {
        let shared = sqlx::query_as::<_, SharedWorkflow>(
            r#"
            SELECT workflow_id, project_id, role, created_at, updated_at
            FROM shared_workflow
            WHERE workflow_id = $1
            "#,
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(shared)
    }

    // =========================================================================
    // Tags
    // =========================================================================

    /// Add a tag to a workflow.
    pub async fn add_tag(&self, workflow_id: &str, tag_id: &str) -> Result<(), DbError> {
        sqlx::query(
            r#"
            INSERT INTO workflow_tag_mapping (workflow_id, tag_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(workflow_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Remove a tag from a workflow.
    pub async fn remove_tag(&self, workflow_id: &str, tag_id: &str) -> Result<bool, DbError> {
        let result = sqlx::query(
            "DELETE FROM workflow_tag_mapping WHERE workflow_id = $1 AND tag_id = $2",
        )
        .bind(workflow_id)
        .bind(tag_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get tags for a workflow.
    pub async fn get_tags(&self, workflow_id: &str) -> Result<Vec<WorkflowTagMapping>, DbError> {
        let tags = sqlx::query_as::<_, WorkflowTagMapping>(
            "SELECT workflow_id, tag_id FROM workflow_tag_mapping WHERE workflow_id = $1",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tags)
    }

    /// Set tags for a workflow (replaces existing).
    pub async fn set_tags(&self, workflow_id: &str, tag_ids: &[String]) -> Result<(), DbError> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Remove existing tags
        sqlx::query("DELETE FROM workflow_tag_mapping WHERE workflow_id = $1")
            .bind(workflow_id)
            .execute(&mut *tx)
            .await?;

        // Add new tags
        for tag_id in tag_ids {
            sqlx::query(
                "INSERT INTO workflow_tag_mapping (workflow_id, tag_id) VALUES ($1, $2)",
            )
            .bind(workflow_id)
            .bind(tag_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

impl WorkflowSharingRole {
    fn to_string(&self) -> String {
        match self {
            Self::Owner => "workflow:owner".to_string(),
            Self::Editor => "workflow:editor".to_string(),
            Self::Viewer => "workflow:viewer".to_string(),
        }
    }
}
