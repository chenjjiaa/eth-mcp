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
use dotenv::dotenv;
use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::TokioChildProcess};
use serde_json::json;
use std::env;
use std::path::PathBuf;
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let wallet_address = env::var("WALLET_ADDRESS")
        .unwrap_or_else(|_| "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string());
    let token_address = env::var("TOKEN_ADDRESS")
        .unwrap_or_else(|_| "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string());

    // Get the path to the server binary
    let server_path = if PathBuf::from("target/debug/server").exists() {
        "target/debug/server"
    } else {
        "target/release/server"
    };

    println!("Starting MCP client...");
    println!("Connecting to server at: {}", server_path);

    // Create client handler (() implements ClientHandler) and start the server as a child process
    let mut cmd = Command::new(server_path);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    // Initialize connection
    println!("\nConnected to server!");
    let server_info = service.peer_info();
    println!("Server info: {:#?}\n", server_info);

    // List available tools
    println!("Listing available tools...");
    let tools = service.list_tools(Default::default()).await?;
    println!("Available tools:");
    for tool in tools.tools {
        println!(
            "  - {}: {}",
            tool.name,
            tool.description.unwrap_or_default()
        );
    }
    println!();

    // Test 1: Get ETH balance (mock address: Vitalik's address)
    println!("Test 1: Getting ETH balance...");
    let eth_balance_result = service
        .call_tool(CallToolRequestParam {
            name: "get_balance".into(),
            arguments: json!({
                "wallet_address": wallet_address
            })
            .as_object()
            .cloned(),
        })
        .await?;

    println!("ETH Balance Result:");
    for content in eth_balance_result.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 2: Get ERC20 token balance (USDC)
    println!("Test 2: Getting USDC token balance...");
    let token_balance_result = service
        .call_tool(CallToolRequestParam {
            name: "get_balance".into(),
            arguments: json!({
                "wallet_address": wallet_address,
                "token_address": token_address
            })
            .as_object()
            .cloned(),
        })
        .await?;

    println!("USDC Balance Result:");
    for content in token_balance_result.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Cleanup
    service.cancel().await?;
    println!("👋 Client disconnected. Goodbye!");

    Ok(())
}
