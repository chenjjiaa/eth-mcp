// Copyright 2025 chenjjiaa
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod server;
mod swap;

use anyhow::{Context, Result};
use dotenv::dotenv;
use rmcp::ServiceExt;
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

use server::EthMcpServer;

async fn run_server(server: EthMcpServer, server_host: String, server_port: u16) -> Result<()> {
    // Determine transport mode: TCP if port is set, otherwise stdio
    if server_port > 0 {
        // TCP mode
        let addr: SocketAddr = format!("{}:{}", server_host, server_port)
            .parse()
            .context("Invalid server address")?;

        let listener = TcpListener::bind(&addr)
            .await
            .context("Failed to bind TCP listener")?;

        let actual_addr = listener
            .local_addr()
            .context("Failed to get local address")?;

        info!(
            "MCP server listening on {}:{}",
            actual_addr.ip(),
            actual_addr.port()
        );

        // Accept connections and serve each one
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!("New connection from {}", peer_addr);
                    let server_clone = server.clone();

                    tokio::spawn(async move {
                        let (read, write) = tokio::io::split(stream);
                        if let Err(e) = server_clone.serve((read, write)).await {
                            info!("Connection {} closed with error: {}", peer_addr, e);
                        } else {
                            info!("Connection {} closed gracefully", peer_addr);
                        }
                    });
                }
                Err(e) => {
                    info!("Failed to accept connection: {}", e);
                }
            }
        }
    } else {
        // Stdio mode (default, for MCP standard)
        info!("Starting MCP server on stdio");
        let running_service = server
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await?;
        // Wait for the service to finish (will wait for client requests)
        running_service.waiting().await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // Get log level from environment or use default
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Initialize tracing with log level from environment or default
    // Use stderr for logs since stdout is used for MCP protocol communication
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr) // Use stderr for logs, stdout is for MCP
        .init();
    info!("Log level: {}", log_level);

    // Get RPC URL from environment or use default public endpoint
    let rpc_url =
        env::var("ETH_RPC_URL").unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
    info!("Starting Ethereum MCP Server with RPC: {}", rpc_url);

    // Get server host and port from environment
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_port = env::var("SERVER_PORT")
        .unwrap_or_else(|_| "0".to_string())
        .parse::<u16>()
        .unwrap_or(0);

    // Create server instance
    let server = EthMcpServer::new(rpc_url)?;

    // Run the server with the specified transport mode
    run_server(server, server_host, server_port).await?;

    Ok(())
}
