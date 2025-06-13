# Multi-stage build for KG MCP Server
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
