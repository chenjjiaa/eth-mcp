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

const USDC_ADDRESS: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let server_path = if PathBuf::from("target/debug/server").exists() {
        "target/debug/server"
    } else if PathBuf::from("target/release/server").exists() {
        "target/release/server"
    } else {
        "target/debug/server"
    };

    println!("Testing Uniswap V2 and V3 Swap Simulation");
    println!("Starting server at: {}\n", server_path);

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

    info!("Listing available tools...");
    match service.list_tools(Default::default()).await {
        Ok(tools) => {
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

    println!("==========================================");
    println!("Uniswap V2 Swap Tests");
    println!("==========================================\n");

    // Test 1: V2 ETH -> USDC
    println!("Test 1: V2 Swap ETH -> USDC");
    println!("Swapping 0.1 ETH for USDC\n");
    let result1 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": "ETH",
                "to_token": USDC_ADDRESS,
                "amount": "0.1",
                "slippage_tolerance": "0.5",
                "version": "v2"
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 1 failed: {}", e);
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

    // Test 2: V2 USDC -> ETH
    println!("Test 2: V2 Swap USDC -> ETH");
    println!("Swapping 100 USDC for ETH\n");
    let result2 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": USDC_ADDRESS,
                "to_token": "ETH",
                "amount": "100",
                "slippage_tolerance": "0.5",
                "version": "v2"
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 2 failed: {}", e);
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

    // Test 3: V2 USDC -> WETH (Token -> Token)
    println!("Test 3: V2 Swap USDC -> WETH");
    println!("Swapping 100 USDC for WETH\n");
    let result3 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": USDC_ADDRESS,
                "to_token": WETH_ADDRESS,
                "amount": "100",
                "slippage_tolerance": "0.5",
                "version": "v2"
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 3 failed: {}", e);
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

    println!("==========================================");
    println!("Uniswap V3 Swap Tests");
    println!("==========================================\n");

    // Test 4: V3 ETH -> USDC (0.05% fee pool)
    println!("Test 4: V3 Swap ETH -> USDC (0.05% fee pool)");
    println!("Swapping 0.1 ETH for USDC\n");
    let result4 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": "ETH",
                "to_token": USDC_ADDRESS,
                "amount": "0.1",
                "slippage_tolerance": "0.5",
                "version": "v3",
                "pool_fee": 500
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 4 failed: {}", e);
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

    // Test 5: V3 USDC -> ETH (0.3% fee pool)
    println!("Test 5: V3 Swap USDC -> ETH (0.3% fee pool)");
    println!("Swapping 100 USDC for ETH\n");
    let result5 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": USDC_ADDRESS,
                "to_token": "ETH",
                "amount": "100",
                "slippage_tolerance": "0.5",
                "version": "v3",
                "pool_fee": 3000
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 5 failed: {}", e);
            service.cancel().await?;
            return Err(e.into());
        }
    };

    println!("Result:");
    for content in result5.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 6: V3 USDC -> WETH (1% fee pool)
    println!("Test 6: V3 Swap USDC -> WETH (1% fee pool)");
    println!("Swapping 100 USDC for WETH\n");
    let result6 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": USDC_ADDRESS,
                "to_token": WETH_ADDRESS,
                "amount": "100",
                "slippage_tolerance": "0.5",
                "version": "v3",
                "pool_fee": 10000
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 6 failed: {}", e);
            service.cancel().await?;
            return Err(e.into());
        }
    };

    println!("Result:");
    for content in result6.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    // Test 7: Default V2 (no version specified)
    println!("Test 7: Default Swap (V2, no version specified)");
    println!("Swapping 0.01 ETH for USDC\n");
    let result7 = match service
        .call_tool(CallToolRequestParam {
            name: "swap_tokens".into(),
            arguments: json!({
                "from_token": "ETH",
                "to_token": USDC_ADDRESS,
                "amount": "0.01",
                "slippage_tolerance": "0.5"
            })
            .as_object()
            .cloned(),
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Test 7 failed: {}", e);
            service.cancel().await?;
            return Err(e.into());
        }
    };

    println!("Result:");
    for content in result7.content {
        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                println!("{}", text.text);
            }
            _ => println!("{:#?}", content),
        }
    }
    println!();

    service.cancel().await?;
    println!("All tests completed!");

    Ok(())
}
