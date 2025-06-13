#!/usr/bin/env cargo
//! # KG MCP Server Setup Utility
//! 
//! This utility helps users quickly set up and configure the Knowledge Graph MCP Server
//! for use with Cursor IDE and other MCP-compatible applications.

use anyhow::Result;
use clap::{Arg, Command};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process;

fn main() -> Result<()> {
    let matches = Command::new("kg-setup")
        .version("0.1.0")
        .author("KG MCP Server Team")
        .about("Setup utility for Knowledge Graph MCP Server")
        .subcommand(
            Command::new("cursor")
                .about("Configure KG MCP Server for Cursor IDE")
                .arg(
                    Arg::new("port")
                        .long("port")
                        .value_name("PORT")
                        .help("Port to run the server on")
                        .default_value("8360")
                )
                .arg(
                    Arg::new("global")
                        .long("global")
                        .help("Install globally for all Cursor projects")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .subcommand(
            Command::new("docker")
                .about("Generate Docker configuration")
                .arg(
                    Arg::new("port")
                        .long("port")
                        .value_name("PORT")
                        .help("External port mapping")
                        .default_value("8360")
                )
        )
        .subcommand(
            Command::new("validate")
                .about("Validate current configuration")
        )
        .subcommand(
            Command::new("start")
                .about("Start the KG MCP Server")
                .arg(
                    Arg::new("daemon")
                        .long("daemon")
                        .help("Run as daemon in background")
                        .action(clap::ArgAction::SetTrue)
                )
        )
        .get_matches();

    match matches.subcommand() {
        Some(("cursor", sub_matches)) => {
            let port = sub_matches.get_one::<String>("port").unwrap();
            let global = sub_matches.get_flag("global");
            setup_cursor(port, global)?;
        }
        Some(("docker", sub_matches)) => {
            let port = sub_matches.get_one::<String>("port").unwrap();
            setup_docker(port)?;
        }
        Some(("validate", _)) => {
            validate_setup()?;
        }
        Some(("start", sub_matches)) => {
            let daemon = sub_matches.get_flag("daemon");
            start_server(daemon)?;
        }
        _ => {
            println!("KG MCP Server Setup Utility");
            println!("Use --help for available commands");
            interactive_setup()?;
        }
    }

    Ok(())
}

fn setup_cursor(port: &str, global: bool) -> Result<()> {
    println!("🚀 Setting up KG MCP Server for Cursor IDE...");
    
    let config = json!({
        "mcpServers": {
            "kg-mcp-server": {
                "url": format!("http://localhost:{}/sse", port)
            }
        }
    });

    let config_path = if global {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/.cursor/mcp.json", home)
    } else {
        ".cursor/mcp.json".to_string()
    };

    // Create directory if it doesn't exist
    if let Some(parent) = Path::new(&config_path).parent() {
        fs::create_dir_all(parent)?;
    }

    // Write configuration
    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    
    println!("✅ Configuration written to: {}", config_path);
    println!("🌐 Server URL: http://localhost:{}/sse", port);
    println!("\n📋 Next steps:");
    println!("1. Start the server: kg-setup start");
    println!("2. Restart Cursor IDE");
    println!("3. Check MCP Servers in Cursor Settings");
    
    Ok(())
}

fn setup_docker(port: &str) -> Result<()> {
    println!("🐳 Generating Docker configuration...");
    
    let dockerfile = format!(r#"# Multi-stage build for KG MCP Server
FROM rust:1.75 AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

# Install dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false kg-server

# Copy binary
COPY --from=builder /app/target/release/kg-mcp-server /usr/local/bin/
RUN chmod +x /usr/local/bin/kg-mcp-server

# Create data directory
RUN mkdir -p /data && chown kg-server:kg-server /data

USER kg-server
WORKDIR /data

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8360/health || exit 1

EXPOSE 8360

ENV MCP_TRANSPORT=sse
ENV MCP_PORT=8360
ENV RUST_LOG=info

CMD ["kg-mcp-server"]
"#);

    let compose = format!(r#"version: '3.8'

services:
  kg-mcp-server:
    build: .
    ports:
      - "{}:8360"
    environment:
      - MCP_TRANSPORT=sse
      - MCP_PORT=8360
      - RUST_LOG=info
    volumes:
      - kg_data:/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8360/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

volumes:
  kg_data:
"#, port);

    fs::write("Dockerfile", dockerfile)?;
    fs::write("docker-compose.yml", compose)?;
    
    println!("✅ Generated Dockerfile and docker-compose.yml");
    println!("🌐 External port: {}", port);
    println!("\n📋 To deploy:");
    println!("docker-compose up -d");
    
    Ok(())
}

fn validate_setup() -> Result<()> {
    println!("🔍 Validating KG MCP Server setup...");
    
    // Check if binary exists
    if which::which("kg-mcp-server").is_ok() {
        println!("✅ kg-mcp-server binary found");
    } else {
        println!("❌ kg-mcp-server binary not found in PATH");
    }
    
    // Check for Cursor config
    let home_config = format!("{}/.cursor/mcp.json", std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    let cursor_configs = vec![
        ".cursor/mcp.json",
        &home_config
    ];
    
    let mut config_found = false;
    for config in cursor_configs {
        if Path::new(config).exists() {
            println!("✅ Cursor config found: {}", config);
            config_found = true;
            break;
        }
    }
    
    if !config_found {
        println!("❌ No Cursor MCP configuration found");
        println!("   Run: kg-setup cursor");
    }
    
    // Test server connection if running
    match std::process::Command::new("curl")
        .args(&["-f", "http://localhost:8360/health"])
        .output()
    {
        Ok(output) if output.status.success() => {
            println!("✅ Server is running and healthy");
        }
        _ => {
            println!("⚠️  Server not running or not responding");
            println!("   Run: kg-setup start");
        }
    }
    
    Ok(())
}

fn start_server(daemon: bool) -> Result<()> {
    println!("🚀 Starting KG MCP Server...");
    
    let mut cmd = process::Command::new("kg-mcp-server");
    
    if daemon {
        println!("🔄 Running in daemon mode...");
        cmd.stdout(process::Stdio::null())
           .stderr(process::Stdio::null());
    }
    
    let mut child = cmd.spawn()?;
    
    if daemon {
        println!("✅ Server started as daemon (PID: {})", child.id());
        println!("🌐 Available at: http://localhost:8360/sse");
    } else {
        println!("✅ Server starting...");
        println!("🌐 Available at: http://localhost:8360/sse");
        println!("Press Ctrl+C to stop");
        child.wait()?;
    }
    
    Ok(())
}

fn interactive_setup() -> Result<()> {
    println!("\n🎯 KG MCP Server Interactive Setup");
    println!("===================================");
    
    println!("\n1. Configure for Cursor IDE");
    println!("2. Generate Docker files");  
    println!("3. Validate setup");
    println!("4. Start server");
    println!("5. Exit");
    
    print!("\nSelect option (1-5): ");
    std::io::Write::flush(&mut std::io::stdout())?;
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    match input.trim() {
        "1" => setup_cursor("8360", false)?,
        "2" => setup_docker("8360")?,
        "3" => validate_setup()?,
        "4" => start_server(false)?,
        "5" => return Ok(()),
        _ => println!("Invalid option"),
    }
    
    Ok(())
} 