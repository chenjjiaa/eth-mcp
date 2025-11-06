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

use anyhow::Result;
use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::TokioChildProcess};
use serde_json::json;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Get the path to the server binary
    let server_path = if PathBuf::from("target/debug/server").exists() {
        "target/debug/server"
    } else {
        "target/release/server"
    };

    println!("Testing ETH Balance Query");
    println!("Starting server at: {}\n", server_path);

    // Create client and start the server as a child process
    let mut cmd = Command::new(server_path);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    info!("Spawning server process...");
    let service = match ().serve(TokioChildProcess::new(cmd)?).await {
        Ok(s) => {
            println!("Connected to server!\n");
            s
        }
        Err(e) => {
            error!("Failed to connect to server: {}", e);
            return Err(e.into());
        }
    };

    // Test: Get ETH balance (Vitalik's address)
    println!("Querying ETH balance for: 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");

    // First, list tools to verify they are registered
    info!("Listing available tools...");
    match service.list_tools(Default::default()).await {
        Ok(tools) => {
            info!("Found {} tools", tools.tools.len());
            for tool in &tools.tools {
                info!(
                    "Tool: {} - {}",
                    tool.name,
                    tool.description.as_deref().unwrap_or("")
                );
            }
            println!("\nAvailable tools: {}", tools.tools.len());
            for tool in &tools.tools {
                println!(
                    "  - {}: {}",
                    tool.name,
                    tool.description.as_deref().unwrap_or("")
                );
            }
            println!();
        }
        Err(e) => {
            error!("Failed to list tools: {}", e);
            service.cancel().await?;
            return Err(e.into());
        }
    }

    info!("Calling get_balance tool...");

    let result = match service
        .call_tool(CallToolRequestParam {
            name: "get_balance".into(),
            arguments: json!({
                "wallet_address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => {
            info!("Tool call succeeded");
            r
        }
        Err(e) => {
            error!("Tool call failed: {}", e);
            service.cancel().await?;
            return Err(e.into());
        }
    };

    println!("\nResult:");
    for content in result.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }

    // Cleanup
    info!("Closing connection...");
    service.cancel().await?;
    println!("\n👋 Test completed!");

    Ok(())
}
