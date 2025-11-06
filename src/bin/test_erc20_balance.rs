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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get the path to the server binary
    let server_path = if PathBuf::from("target/debug/server").exists() {
        "target/debug/server"
    } else {
        "target/release/server"
    };

    println!("Testing ERC20 Token Balance Query");
    println!("Starting server at: {}\n", server_path);

    // Create client and start the server as a child process
    let mut cmd = Command::new(server_path);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    println!("Connected to server!\n");

    // Test: Get USDC token balance
    println!("Querying USDC balance for: 0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
    println!("   Token contract: 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 (USDC)\n");

    let result = service
        .call_tool(CallToolRequestParam {
            name: "get_balance".into(),
            arguments: json!({
                "wallet_address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
                "token_address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            })
            .as_object()
            .cloned(),
        })
        .await?;

    println!("Result:");
    for content in result.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }

    // Cleanup
    service.cancel().await?;
    println!("\n👋 Test completed!");

    Ok(())
}
