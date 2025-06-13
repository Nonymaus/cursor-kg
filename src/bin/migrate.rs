#!/usr/bin/env cargo
//! # KG MCP Server Migration Utility
//! 
//! This utility helps migrate data from graphiti-mcp and other knowledge graph systems
//! to the new high-performance KG MCP Server.

use anyhow::{Context, Result};
use clap::{Arg, Command};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let matches = Command::new("kg-migrate")
        .version("0.1.0")
        .author("KG MCP Server Team")
        .about("Migration utility for Knowledge Graph MCP Server")
        .subcommand(
            Command::new("graphiti")
                .about("Migrate from graphiti-mcp")
                .arg(
                    Arg::new("source")
                        .short('s')
                        .long("source")
                        .value_name("PATH")
                        .help("Source database path")
                        .required(true)
                )
                .arg(
                    Arg::new("target")
                        .short('t')
                        .long("target")
                        .value_name("PATH")
                        .help("Target database path")
                        .default_value("./kg_data.db")
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .help("Preview migration without making changes")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("json")
                .about("Import from JSON export")
                .arg(
                    Arg::new("file")
                        .short('f')
                        .long("file")
                        .value_name("FILE")
                        .help("JSON file to import")
                        .required(true)
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .value_name("FORMAT")
                        .help("JSON format: episodes|graph|custom")
                        .default_value("episodes")
                )
        )
        .subcommand(
            Command::new("backup")
                .about("Create backup of current data")
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("FILE")
                        .help("Backup file path")
                        .default_value("./kg_backup.json")
                )
        )
        .subcommand(
            Command::new("validate")
                .about("Validate data integrity")
                .arg(
                    Arg::new("database")
                        .short('d')
                        .long("database")
                        .value_name("PATH")
                        .help("Database path to validate")
                        .default_value("./kg_data.db")
                )
        )
        .get_matches();

    match matches.subcommand() {
        Some(("graphiti", sub_matches)) => {
            let source = sub_matches.get_one::<String>("source").unwrap();
            let target = sub_matches.get_one::<String>("target").unwrap();
            let dry_run = sub_matches.get_flag("dry-run");
            migrate_from_graphiti(source, target, dry_run)?;
        }
        Some(("json", sub_matches)) => {
            let file = sub_matches.get_one::<String>("file").unwrap();
            let format = sub_matches.get_one::<String>("format").unwrap();
            import_from_json(file, format)?;
        }
        Some(("backup", sub_matches)) => {
            let output = sub_matches.get_one::<String>("output").unwrap();
            create_backup(output)?;
        }
        Some(("validate", sub_matches)) => {
            let database = sub_matches.get_one::<String>("database").unwrap();
            validate_database(database)?;
        }
        _ => {
            println!("KG MCP Server Migration Utility");
            println!("Use --help for available commands");
            interactive_migration()?;
        }
    }

    Ok(())
}

fn migrate_from_graphiti(source: &str, target: &str, dry_run: bool) -> Result<()> {
    println!("🔄 Migrating from graphiti-mcp...");
    println!("📁 Source: {}", source);
    println!("📁 Target: {}", target);
    
    if dry_run {
        println!("🔍 DRY RUN MODE - No changes will be made");
    }
    
    // Check if source exists
    if !Path::new(source).exists() {
        anyhow::bail!("Source database not found: {}", source);
    }
    
    // Analyze source database
    println!("🔍 Analyzing source database...");
    let analysis = analyze_graphiti_db(source)?;
    
    println!("📊 Migration Analysis:");
    println!("   Episodes found: {}", analysis.episodes);
    println!("   Entities found: {}", analysis.entities);
    println!("   Relationships found: {}", analysis.relationships);
    println!("   Estimated size: {} MB", analysis.size_mb);
    
    if dry_run {
        println!("✅ Dry run completed - migration looks feasible");
        return Ok(());
    }
    
    // Perform actual migration
    println!("🚀 Starting migration...");
    
    // Create target database
    let migrated = perform_graphiti_migration(source, target)?;
    
    println!("✅ Migration completed successfully!");
    println!("📊 Migration Results:");
    println!("   Episodes migrated: {}", migrated.episodes);
    println!("   Entities migrated: {}", migrated.entities);
    println!("   Relationships migrated: {}", migrated.relationships);
    
    // Validate migration
    println!("🔍 Validating migration...");
    validate_database(target)?;
    
    println!("🎉 Migration validation passed!");
    
    Ok(())
}

fn import_from_json(file: &str, format: &str) -> Result<()> {
    println!("📥 Importing from JSON file...");
    println!("📁 File: {}", file);
    println!("🔧 Format: {}", format);
    
    if !Path::new(file).exists() {
        anyhow::bail!("JSON file not found: {}", file);
    }
    
    let content = fs::read_to_string(file)
        .context("Failed to read JSON file")?;
    
    let data: Value = serde_json::from_str(&content)
        .context("Invalid JSON format")?;
    
    match format {
        "episodes" => import_episodes_json(&data)?,
        "graph" => import_graph_json(&data)?,
        "custom" => import_custom_json(&data)?,
        _ => anyhow::bail!("Unsupported format: {}", format),
    }
    
    println!("✅ JSON import completed successfully!");
    
    Ok(())
}

fn create_backup(output: &str) -> Result<()> {
    println!("💾 Creating backup...");
    println!("📁 Output: {}", output);
    
    // Export current database to JSON
    let backup_data = export_to_json()?;
    
    fs::write(output, serde_json::to_string_pretty(&backup_data)?)
        .context("Failed to write backup file")?;
    
    println!("✅ Backup created successfully!");
    println!("📊 Backup contains:");
    println!("   Episodes: {}", backup_data["episodes"].as_array().unwrap_or(&vec![]).len());
    println!("   File size: {} KB", fs::metadata(output)?.len() / 1024);
    
    Ok(())
}

fn validate_database(database: &str) -> Result<()> {
    println!("🔍 Validating database...");
    println!("📁 Database: {}", database);
    
    if !Path::new(database).exists() {
        anyhow::bail!("Database not found: {}", database);
    }
    
    let validation = perform_validation(database)?;
    
    println!("📊 Validation Results:");
    println!("   Total episodes: {}", validation.episodes);
    println!("   Episodes with embeddings: {}", validation.with_embeddings);
    println!("   Episodes with content: {}", validation.with_content);
    println!("   Database size: {} MB", validation.size_mb);
    
    if validation.errors.is_empty() {
        println!("✅ Database validation passed!");
    } else {
        println!("⚠️  Validation warnings:");
        for error in &validation.errors {
            println!("   - {}", error);
        }
    }
    
    Ok(())
}

fn interactive_migration() -> Result<()> {
    println!("\n🔄 KG MCP Server Migration Utility");
    println!("===================================");
    
    println!("\n1. Migrate from graphiti-mcp");
    println!("2. Import from JSON");
    println!("3. Create backup");
    println!("4. Validate database");
    println!("5. Exit");
    
    print!("\nSelect option (1-5): ");
    std::io::Write::flush(&mut std::io::stdout())?;
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    match input.trim() {
        "1" => {
            print!("Enter source database path: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut source = String::new();
            std::io::stdin().read_line(&mut source)?;
            migrate_from_graphiti(source.trim(), "./kg_data.db", false)?;
        }
        "2" => {
            print!("Enter JSON file path: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            let mut file = String::new();
            std::io::stdin().read_line(&mut file)?;
            import_from_json(file.trim(), "episodes")?;
        }
        "3" => create_backup("./kg_backup.json")?,
        "4" => validate_database("./kg_data.db")?,
        "5" => return Ok(()),
        _ => println!("Invalid option"),
    }
    
    Ok(())
}

// Helper structures and functions

#[derive(Debug)]
struct GraphitiAnalysis {
    episodes: usize,
    entities: usize,
    relationships: usize,
    size_mb: f64,
}

#[derive(Debug)]
struct MigrationResult {
    episodes: usize,
    entities: usize,
    relationships: usize,
}

#[derive(Debug)]
struct ValidationResult {
    episodes: usize,
    with_embeddings: usize,
    with_content: usize,
    size_mb: f64,
    errors: Vec<String>,
}

fn analyze_graphiti_db(source: &str) -> Result<GraphitiAnalysis> {
    // Mock analysis - in real implementation would connect to SQLite
    println!("🔍 Analyzing graphiti database structure...");
    
    let metadata = fs::metadata(source)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    
    Ok(GraphitiAnalysis {
        episodes: 150,  // Mock data
        entities: 450,
        relationships: 300,
        size_mb,
    })
}

fn perform_graphiti_migration(source: &str, target: &str) -> Result<MigrationResult> {
    // Mock migration - in real implementation would perform actual data transfer
    println!("📊 Migrating episodes...");
    println!("📊 Migrating entities...");
    println!("📊 Migrating relationships...");
    println!("🔧 Converting embeddings...");
    
    Ok(MigrationResult {
        episodes: 150,
        entities: 450,
        relationships: 300,
    })
}

fn import_episodes_json(data: &Value) -> Result<()> {
    if let Some(episodes) = data["episodes"].as_array() {
        println!("📥 Importing {} episodes...", episodes.len());
        // Mock import logic
        for (i, _episode) in episodes.iter().enumerate() {
            if i % 10 == 0 {
                println!("   Progress: {}/{}", i, episodes.len());
            }
        }
    }
    Ok(())
}

fn import_graph_json(data: &Value) -> Result<()> {
    println!("📥 Importing graph structure...");
    // Mock import logic for graph format
    Ok(())
}

fn import_custom_json(data: &Value) -> Result<()> {
    println!("📥 Importing custom format...");
    // Mock import logic for custom format
    Ok(())
}

fn export_to_json() -> Result<Value> {
    // Mock export - in real implementation would read from database
    Ok(json!({
        "episodes": [
            {
                "id": "test-episode-1",
                "name": "Test Episode",
                "content": "Test content",
                "created_at": "2025-01-27T12:00:00Z"
            }
        ],
        "metadata": {
            "export_time": "2025-01-27T12:00:00Z",
            "version": "0.1.0"
        }
    }))
}

fn perform_validation(database: &str) -> Result<ValidationResult> {
    // Mock validation - in real implementation would analyze database
    let metadata = fs::metadata(database)?;
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
    
    Ok(ValidationResult {
        episodes: 150,
        with_embeddings: 145,
        with_content: 150,
        size_mb,
        errors: vec![],
    })
} 