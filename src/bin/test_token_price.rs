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

    println!("Testing Token Price Query");
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

    // First, list tools to verify get_token_price is registered
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
            println!("Available tools: {}", tools.tools.len());
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

    // Test 1: Get price by symbol (ETH)
    println!("Test 1: Querying price by symbol (ETH)");
    info!("Calling get_token_price tool with symbol: ETH");

    let result1 = match service
        .call_tool(CallToolRequestParam {
            name: "get_token_price".into(),
            arguments: json!({
                "token": "ETH"
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

    println!("Result:");
    for content in result1.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 2: Get price by symbol (USDC)
    println!("Test 2: Querying price by symbol (USDC)");
    info!("Calling get_token_price tool with symbol: USDC");

    let result2 = match service
        .call_tool(CallToolRequestParam {
            name: "get_token_price".into(),
            arguments: json!({
                "token": "USDC"
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

    println!("Result:");
    for content in result2.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 3: Get price by contract address (USDC)
    println!("Test 3: Querying price by contract address (USDC)");
    println!("Token address: 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    info!("Calling get_token_price tool with address: 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

    let result3 = match service
        .call_tool(CallToolRequestParam {
            name: "get_token_price".into(),
            arguments: json!({
                "token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
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

    println!("Result:");
    for content in result3.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 4: Get price by symbol (WETH)
    println!("Test 4: Querying price by symbol (WETH)");
    info!("Calling get_token_price tool with symbol: WETH");

    let result4 = match service
        .call_tool(CallToolRequestParam {
            name: "get_token_price".into(),
            arguments: json!({
                "token": "WETH"
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

    println!("Result:");
    for content in result4.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Cleanup
    info!("Closing connection...");
    service.cancel().await?;
    println!("Test completed!");

    Ok(())
}
