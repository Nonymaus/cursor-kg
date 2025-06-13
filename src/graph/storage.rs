use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags, Row, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::{Episode, KGEdge, KGNode, SearchResult};
use crate::config::DatabaseConfig;

#[derive(Clone)]
pub struct GraphStorage {
    conn: Arc<Mutex<Connection>>,
}

impl GraphStorage {
    pub fn new(db_path: &Path, config: &DatabaseConfig) -> Result<Self> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

        // Configure SQLite for performance
        conn.execute_batch(&format!(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = {};
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -{};
            PRAGMA temp_store = memory;
            PRAGMA mmap_size = 268435456;
            ",
            if config.enable_wal { "WAL" } else { "DELETE" },
            config.cache_size_kb
        ))?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.initialize_schema()?;
        Ok(storage)
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create nodes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                uuid TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                node_type TEXT NOT NULL,
                summary TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                group_id TEXT,
                metadata TEXT DEFAULT '{}'
            )",
            [],
        )?;

        // Create edges table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS edges (
                uuid TEXT PRIMARY KEY,
                source_node_uuid TEXT NOT NULL,
                target_node_uuid TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                summary TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                group_id TEXT,
                metadata TEXT DEFAULT '{}',
                FOREIGN KEY (source_node_uuid) REFERENCES nodes (uuid),
                FOREIGN KEY (target_node_uuid) REFERENCES nodes (uuid)
            )",
            [],
        )?;

        // Create episodes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS episodes (
                uuid TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                source TEXT NOT NULL,
                source_description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                group_id TEXT,
                metadata TEXT DEFAULT '{}'
            )",
            [],
        )?;

        // Create embeddings table for vector storage
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embeddings (
                uuid TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL, -- 'node', 'edge', 'episode'
                embedding BLOB NOT NULL,
                dimensions INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (uuid) REFERENCES nodes (uuid) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create episode_entities junction table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS episode_entities (
                episode_uuid TEXT NOT NULL,
                entity_uuid TEXT NOT NULL,
                entity_type TEXT NOT NULL, -- 'node' or 'edge'
                PRIMARY KEY (episode_uuid, entity_uuid),
                FOREIGN KEY (episode_uuid) REFERENCES episodes (uuid) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create FTS5 virtual table for full-text search
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
                uuid UNINDEXED,
                name,
                summary,
                content='nodes',
                content_rowid='rowid'
            )",
            [],
        )?;

        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts USING fts5(
                uuid UNINDEXED,
                name,
                content,
                content='episodes',
                content_rowid='rowid'
            )",
            [],
        )?;

        // Create indices for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_group_id ON nodes (group_id)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes (node_type)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_source ON edges (source_node_uuid)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_target ON edges (target_node_uuid)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_group_id ON edges (group_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_episodes_group_id ON episodes (group_id)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_episodes_created_at ON episodes (created_at)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_type ON embeddings (entity_type)",
            [],
        )?;

        // Create triggers to keep FTS tables in sync
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS nodes_fts_insert AFTER INSERT ON nodes
            BEGIN
                INSERT INTO nodes_fts(uuid, name, summary) VALUES (new.uuid, new.name, new.summary);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS nodes_fts_update AFTER UPDATE ON nodes
            BEGIN
                UPDATE nodes_fts SET name = new.name, summary = new.summary WHERE uuid = new.uuid;
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS nodes_fts_delete AFTER DELETE ON nodes
            BEGIN
                DELETE FROM nodes_fts WHERE uuid = old.uuid;
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS episodes_fts_insert AFTER INSERT ON episodes
            BEGIN
                INSERT INTO episodes_fts(uuid, name, content) VALUES (new.uuid, new.name, new.content);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS episodes_fts_update AFTER UPDATE ON episodes
            BEGIN
                UPDATE episodes_fts SET name = new.name, content = new.content WHERE uuid = new.uuid;
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS episodes_fts_delete AFTER DELETE ON episodes
            BEGIN
                DELETE FROM episodes_fts WHERE uuid = old.uuid;
            END",
            [],
        )?;

        Ok(())
    }

    pub fn insert_node(&self, node: &KGNode) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO nodes 
             (uuid, name, node_type, summary, created_at, updated_at, group_id, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                node.uuid.to_string(),
                node.name,
                node.node_type,
                node.summary,
                node.created_at.to_rfc3339(),
                node.updated_at.to_rfc3339(),
                node.group_id,
                serde_json::to_string(&node.metadata)?
            ],
        )?;
        Ok(())
    }

    pub fn insert_edge(&self, edge: &KGEdge) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO edges 
             (uuid, source_node_uuid, target_node_uuid, relation_type, summary, weight, created_at, updated_at, group_id, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                edge.uuid.to_string(),
                edge.source_node_uuid.to_string(),
                edge.target_node_uuid.to_string(),
                edge.relation_type,
                edge.summary,
                edge.weight,
                edge.created_at.to_rfc3339(),
                edge.updated_at.to_rfc3339(),
                edge.group_id,
                serde_json::to_string(&edge.metadata)?
            ],
        )?;
        Ok(())
    }

    pub fn insert_episode(&self, episode: &Episode) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Insert episode
        conn.execute(
            "INSERT OR REPLACE INTO episodes 
             (uuid, name, content, source, source_description, created_at, group_id, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                episode.uuid.to_string(),
                episode.name,
                episode.content,
                serde_json::to_string(&episode.source)?,
                episode.source_description,
                episode.created_at.to_rfc3339(),
                episode.group_id,
                serde_json::to_string(&episode.metadata)?
            ],
        )?;

        // Insert episode-entity relationships
        for entity_uuid in &episode.entity_uuids {
            conn.execute(
                "INSERT OR IGNORE INTO episode_entities (episode_uuid, entity_uuid, entity_type)
                 VALUES (?1, ?2, 'node')",
                params![episode.uuid.to_string(), entity_uuid.to_string()],
            )?;
        }

        for edge_uuid in &episode.edge_uuids {
            conn.execute(
                "INSERT OR IGNORE INTO episode_entities (episode_uuid, entity_uuid, entity_type)
                 VALUES (?1, ?2, 'edge')",
                params![episode.uuid.to_string(), edge_uuid.to_string()],
            )?;
        }

        Ok(())
    }

    pub fn store_embedding(&self, entity_uuid: Uuid, entity_type: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Convert f32 slice to bytes
        let embedding_bytes: Vec<u8> = embedding
            .iter()
            .flat_map(|&f| f.to_le_bytes().to_vec())
            .collect();

        conn.execute(
            "INSERT OR REPLACE INTO embeddings (uuid, entity_type, embedding, dimensions, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entity_uuid.to_string(),
                entity_type,
                embedding_bytes,
                embedding.len(),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        Ok(())
    }

    pub fn get_node(&self, uuid: Uuid) -> Result<Option<KGNode>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT uuid, name, node_type, summary, created_at, updated_at, group_id, metadata
             FROM nodes WHERE uuid = ?1"
        )?;

        let node = stmt.query_row(params![uuid.to_string()], |row| {
            self.row_to_node(row)
        }).optional()?;

        Ok(node)
    }

    pub fn get_edge(&self, uuid: Uuid) -> Result<Option<KGEdge>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT uuid, source_node_uuid, target_node_uuid, relation_type, summary, weight, created_at, updated_at, group_id, metadata
             FROM edges WHERE uuid = ?1"
        )?;

        let edge = stmt.query_row(params![uuid.to_string()], |row| {
            self.row_to_edge(row)
        }).optional()?;

        Ok(edge)
    }

    pub fn get_episode(&self, uuid: Uuid) -> Result<Option<Episode>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT uuid, name, content, source, source_description, created_at, group_id, metadata
             FROM episodes WHERE uuid = ?1"
        )?;

        let mut episode = stmt.query_row(params![uuid.to_string()], |row| {
            self.row_to_episode(row)
        }).optional()?;

        if let Some(ref mut ep) = episode {
            // Load associated entities
            let mut entity_stmt = conn.prepare(
                "SELECT entity_uuid, entity_type FROM episode_entities WHERE episode_uuid = ?1"
            )?;

            let entities: Result<Vec<_>, anyhow::Error> = entity_stmt.query_map(params![uuid.to_string()], |row| {
                let uuid_str: String = row.get(0)?;
                let entity_type: String = row.get(1)?;
                let entity_uuid = Uuid::parse_str(&uuid_str)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                Ok((entity_uuid, entity_type))
            })?.collect::<Result<Vec<_>, rusqlite::Error>>()
            .map_err(|e| anyhow::Error::new(e));

            for (entity_uuid, entity_type) in entities? {
                match entity_type.as_str() {
                    "node" => ep.add_entity(entity_uuid),
                    "edge" => ep.add_edge(entity_uuid),
                    _ => {}
                }
            }

            // Load embedding if exists
            let mut emb_stmt = conn.prepare(
                "SELECT embedding, dimensions FROM embeddings WHERE uuid = ?1 AND entity_type = 'episode'"
            )?;

            if let Ok(embedding_data) = emb_stmt.query_row(params![uuid.to_string()], |row| {
                let embedding_bytes: Vec<u8> = row.get(0)?;
                let dimensions: usize = row.get(1)?;
                Ok((embedding_bytes, dimensions))
            }) {
                let (embedding_bytes, dimensions) = embedding_data;
                let embedding: Vec<f32> = embedding_bytes
                    .chunks(4)
                    .take(dimensions)
                    .map(|chunk| {
                        let mut bytes = [0u8; 4];
                        bytes.copy_from_slice(chunk);
                        f32::from_le_bytes(bytes)
                    })
                    .collect();
                ep.set_embedding(embedding);
            }
        }

        Ok(episode)
    }

    pub fn search_nodes_by_text(&self, query: &str, group_id: Option<&str>, limit: usize) -> Result<Vec<KGNode>> {
        let conn = self.conn.lock().unwrap();
        
        if let Some(group_id) = group_id {
            let mut stmt = conn.prepare(
                "SELECT n.uuid, n.name, n.node_type, n.summary, n.created_at, n.updated_at, n.group_id, n.metadata
                 FROM nodes_fts f
                 JOIN nodes n ON f.uuid = n.uuid
                 WHERE nodes_fts MATCH ?1 AND n.group_id = ?2
                 ORDER BY rank
                 LIMIT ?3"
            )?;
            let rows = stmt.query_map(params![query, group_id, limit], |row| self.row_to_node(row))?;
            let nodes: Result<Vec<_>, anyhow::Error> = rows.collect::<Result<Vec<_>, rusqlite::Error>>()
                .map_err(|e| anyhow::Error::new(e));
            Ok(nodes?)
        } else {
            let mut stmt = conn.prepare(
                "SELECT n.uuid, n.name, n.node_type, n.summary, n.created_at, n.updated_at, n.group_id, n.metadata
                 FROM nodes_fts f
                 JOIN nodes n ON f.uuid = n.uuid
                 WHERE nodes_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(params![query, limit], |row| self.row_to_node(row))?;
            let nodes: Result<Vec<_>, anyhow::Error> = rows.collect::<Result<Vec<_>, rusqlite::Error>>()
                .map_err(|e| anyhow::Error::new(e));
            Ok(nodes?)
        }
    }

    pub fn get_recent_episodes(&self, group_id: Option<&str>, limit: usize) -> Result<Vec<Episode>> {
        let conn = self.conn.lock().unwrap();
        let mut query = "SELECT uuid, name, content, source, source_description, created_at, group_id, metadata
                         FROM episodes".to_string();
        
        let mut params = Vec::new();
        if let Some(gid) = group_id {
            query.push_str(" WHERE group_id = ?");
            params.push(gid.to_string());
        }
        
        query.push_str(" ORDER BY created_at DESC LIMIT ?");
        params.push(limit.to_string());

        let mut stmt = conn.prepare(&query)?;
        let episode_iter = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            self.row_to_episode(row)
        })?;

        let mut episodes = Vec::new();
        for episode in episode_iter {
            episodes.push(episode?);
        }

        Ok(episodes)
    }

    pub async fn count_nodes(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub async fn count_edges(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub async fn count_episodes(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM episodes", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn search_episodes_by_content(&self, query: &str, limit: usize) -> Result<Vec<Episode>> {
        let conn = self.conn.lock().unwrap();
        
        // Simple content search using LIKE for now
        let mut stmt = conn.prepare(
            "SELECT uuid, name, content, source, source_description, created_at, group_id, metadata
             FROM episodes
             WHERE content LIKE ?1 OR name LIKE ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;
        
        let search_pattern = format!("%{}%", query);
        let episode_iter = stmt.query_map(params![search_pattern, limit], |row| {
            self.row_to_episode(row)
        })?;

        let mut episodes = Vec::new();
        for episode in episode_iter {
            episodes.push(episode?);
        }

        Ok(episodes)
    }

    /// Search edges by text in relation_type and summary fields
    pub fn search_edges_by_text(&self, query: &str, limit: usize) -> Result<Vec<KGEdge>> {
        let conn = self.conn.lock().unwrap();
        
        // Search edges by relation_type and summary using LIKE
        let mut stmt = conn.prepare(
            "SELECT uuid, source_node_uuid, target_node_uuid, relation_type, summary, weight, created_at, updated_at, group_id, metadata
             FROM edges
             WHERE relation_type LIKE ?1 OR summary LIKE ?1
             ORDER BY weight DESC, created_at DESC
             LIMIT ?2"
        )?;
        
        let search_pattern = format!("%{}%", query);
        let edge_iter = stmt.query_map(params![search_pattern, limit], |row| {
            self.row_to_edge(row)
        })?;

        let mut edges = Vec::new();
        for edge in edge_iter {
            edges.push(edge?);
        }

        Ok(edges)
    }

    /// Get edges between two specific nodes
    pub fn get_edges_between_nodes(&self, source_uuid: Uuid, target_uuid: Uuid) -> Result<Vec<KGEdge>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT uuid, source_node_uuid, target_node_uuid, relation_type, summary, weight, created_at, updated_at, group_id, metadata
             FROM edges
             WHERE source_node_uuid = ?1 AND target_node_uuid = ?2
             ORDER BY weight DESC, created_at DESC"
        )?;
        
        let edge_iter = stmt.query_map(params![source_uuid.to_string(), target_uuid.to_string()], |row| {
            self.row_to_edge(row)
        })?;

        let mut edges = Vec::new();
        for edge in edge_iter {
            edges.push(edge?);
        }

        Ok(edges)
    }

    /// Delete an episode and its associated data
    pub fn delete_episode(&self, uuid: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Delete episode-entity relationships first (due to foreign key constraints)
        conn.execute(
            "DELETE FROM episode_entities WHERE episode_uuid = ?1",
            params![uuid.to_string()],
        )?;
        
        // Delete embedding if exists
        conn.execute(
            "DELETE FROM embeddings WHERE uuid = ?1 AND entity_type = 'episode'",
            params![uuid.to_string()],
        )?;
        
        // Delete the episode itself (FTS trigger will handle nodes_fts cleanup)
        let deleted = conn.execute(
            "DELETE FROM episodes WHERE uuid = ?1",
            params![uuid.to_string()],
        )?;
        
        if deleted == 0 {
            return Err(anyhow::anyhow!("Episode with UUID {} not found", uuid));
        }
        
        Ok(())
    }

    /// Delete an edge and its associated data
    pub fn delete_edge(&self, uuid: &Uuid) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Delete embedding if exists
        conn.execute(
            "DELETE FROM embeddings WHERE uuid = ?1 AND entity_type = 'edge'",
            params![uuid.to_string()],
        )?;
        
        // Delete episode-entity relationships
        conn.execute(
            "DELETE FROM episode_entities WHERE entity_uuid = ?1 AND entity_type = 'edge'",
            params![uuid.to_string()],
        )?;
        
        // Delete the edge itself
        let deleted = conn.execute(
            "DELETE FROM edges WHERE uuid = ?1",
            params![uuid.to_string()],
        )?;
        
        if deleted == 0 {
            return Err(anyhow::anyhow!("Edge with UUID {} not found", uuid));
        }
        
        Ok(())
    }

    /// Clear all data from the database (destructive operation)
    pub fn clear_all_data(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Disable foreign key constraints temporarily
        conn.execute("PRAGMA foreign_keys = OFF", [])?;
        
        // Clear all tables in correct order to avoid constraint violations
        conn.execute("DELETE FROM episode_entities", [])?;
        conn.execute("DELETE FROM embeddings", [])?;
        conn.execute("DELETE FROM episodes", [])?;
        conn.execute("DELETE FROM edges", [])?;
        conn.execute("DELETE FROM nodes", [])?;
        
        // Clear FTS tables (triggers should handle this, but be explicit)
        conn.execute("DELETE FROM episodes_fts", [])?;
        conn.execute("DELETE FROM nodes_fts", [])?;
        
        // Re-enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        
        // Vacuum to reclaim space
        conn.execute("VACUUM", [])?;
        
        Ok(())
    }

    fn row_to_node(&self, row: &Row) -> rusqlite::Result<KGNode> {
        Ok(KGNode {
            uuid: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
            name: row.get(1)?,
            node_type: row.get(2)?,
            summary: row.get(3)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&chrono::Utc),
            group_id: row.get(6)?,
            metadata: serde_json::from_str(&row.get::<_, String>(7)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?,
        })
    }

    fn row_to_edge(&self, row: &Row) -> rusqlite::Result<KGEdge> {
        Ok(KGEdge {
            uuid: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
            source_node_uuid: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?,
            target_node_uuid: Uuid::parse_str(&row.get::<_, String>(2)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?,
            relation_type: row.get(3)?,
            summary: row.get(4)?,
            weight: row.get(5)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&chrono::Utc),
            group_id: row.get(8)?,
            metadata: serde_json::from_str(&row.get::<_, String>(9)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e)))?,
        })
    }

    fn row_to_episode(&self, row: &Row) -> rusqlite::Result<Episode> {
        Ok(Episode {
            uuid: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
            name: row.get(1)?,
            content: row.get(2)?,
            source: serde_json::from_str(&row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?,
            source_description: row.get(4)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                .with_timezone(&chrono::Utc),
            group_id: row.get(6)?,
            entity_uuids: Vec::new(),
            edge_uuids: Vec::new(),
            embedding: None,
            metadata: serde_json::from_str(&row.get::<_, String>(7)?)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?,
        })
    }
} 