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

use anyhow::{Context, Result};
use ethabi::{Function, Param, ParamType, StateMutability, Token};
use ethers::{
    prelude::*,
    types::{Address, TransactionRequest},
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, instrument};

use crate::swap::{SwapInput, SwapProvider};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetBalanceInput {
    /// Wallet address to query
    pub wallet_address: String,
    /// Optional ERC20 token contract address. If not provided, returns ETH balance
    #[serde(default)]
    pub token_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceOutput {
    /// Wallet address
    pub wallet_address: String,
    /// Token address (null for ETH)
    pub token_address: Option<String>,
    /// Balance as a string with proper decimals
    pub balance: String,
    /// Number of decimals
    pub decimals: u8,
    /// Raw balance (wei or token units)
    pub raw_balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetTokenPriceInput {
    /// Token contract address (0x...) or symbol (e.g., "USDC", "WETH")
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPriceOutput {
    /// Token identifier (address or symbol)
    pub token: String,
    /// Token address if available
    pub token_address: Option<String>,
    /// Price in USD
    pub price_usd: Option<String>,
    /// Price in ETH
    pub price_eth: Option<String>,
    /// Last updated timestamp
    pub last_updated: Option<String>,
}

#[derive(Clone)]
pub struct EthMcpServer {
    provider: Arc<Provider<Http>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl EthMcpServer {
    pub fn new(rpc_url: String) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url.as_str())
            .context("Failed to create HTTP provider")?;

        let tool_router = Self::tool_router();
        info!("Tool router initialized");

        Ok(Self {
            provider: Arc::new(provider),
            tool_router,
        })
    }

    #[tool(description = "Query ETH and ERC20 token balances for a wallet address")]
    #[instrument(skip(self))]
    async fn get_balance(
        &self,
        params: Parameters<GetBalanceInput>,
    ) -> Result<CallToolResult, McpError> {
        info!("get_balance called with params: {:?}", params.0);
        let input = params.0;
        let wallet_address = Address::from_str(&input.wallet_address).map_err(|e| {
            McpError::invalid_params(format!("Invalid wallet address: {}", e), None)
        })?;

        info!(
            "Querying balance for wallet: {:?}, token: {:?}",
            wallet_address, input.token_address
        );

        let result = if let Some(token_address_str) = input.token_address {
            // Query ERC20 token balance
            info!("Querying ERC20 balance");
            self.get_erc20_balance(wallet_address, token_address_str)
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("Failed to get ERC20 balance: {}", e), None)
                })?
        } else {
            // Query ETH balance
            info!("Querying ETH balance");
            self.get_eth_balance(wallet_address).await.map_err(|e| {
                McpError::internal_error(format!("Failed to get ETH balance: {}", e), None)
            })?
        };

        info!("Balance query completed, serializing result");
        let json_result = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("Error serializing result: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json_result)]))
    }

    #[tool(
        description = "Get current token price in USD and ETH. Accepts token contract address or symbol"
    )]
    #[instrument(skip(self))]
    async fn get_token_price(
        &self,
        params: Parameters<GetTokenPriceInput>,
    ) -> Result<CallToolResult, McpError> {
        info!("get_token_price called with params: {:?}", params.0);
        let input = params.0;

        info!("Fetching price for token: {}", input.token);

        let result = self.fetch_token_price(&input.token).await.map_err(|e| {
            McpError::internal_error(format!("Failed to get token price: {}", e), None)
        })?;

        info!("Price query completed, serializing result");
        let json_result = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("Error serializing result: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json_result)]))
    }

    #[tool(
        description = "Simulate a token swap on Uniswap V2. Constructs a real transaction and simulates it using eth_call without executing on-chain. Returns estimated output and gas costs."
    )]
    #[instrument(skip(self))]
    async fn swap_tokens(&self, params: Parameters<SwapInput>) -> Result<CallToolResult, McpError> {
        info!("swap_tokens called with params: {:?}", params.0);
        let input = params.0;

        info!(
            "Simulating swap: {} -> {} (amount: {}, slippage: {}%)",
            input.from_token, input.to_token, input.amount, input.slippage_tolerance
        );

        let provider = SwapProvider::new(self.provider.clone());
        let result = provider.estimate_swap(input).await.map_err(|e| {
            McpError::internal_error(format!("Failed to estimate swap: {}", e), None)
        })?;

        info!("Swap simulation completed, serializing result");
        let json_result = serde_json::to_string_pretty(&result).map_err(|e| {
            McpError::internal_error(format!("Error serializing result: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(json_result)]))
    }

    #[instrument(skip(self))]
    async fn fetch_token_price(&self, token: &str) -> Result<TokenPriceOutput> {
        let client = reqwest::Client::new();
        let token_lower = token.to_lowercase();

        // Check if input is an Ethereum address (starts with 0x)
        let is_address = token_lower.starts_with("0x") && token_lower.len() == 42;

        let result = if is_address {
            // Query by contract address
            self.fetch_price_by_address(&client, &token_lower).await?
        } else if token_lower == "eth" || token_lower == "ethereum" {
            // Special case for ETH
            self.fetch_eth_price(&client).await?
        } else {
            // Query by symbol
            self.fetch_price_by_symbol(&client, &token_lower).await?
        };

        Ok(result)
    }

    async fn fetch_price_by_address(
        &self,
        client: &reqwest::Client,
        address: &str,
    ) -> Result<TokenPriceOutput> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/token_price/ethereum?contract_addresses={}&vs_currencies=usd,eth",
            address
        );

        info!("Fetching price by address from CoinGecko: {}", url);

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to CoinGecko")?;

        if !response.status().is_success() {
            anyhow::bail!("CoinGecko API returned error: {}", response.status());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse CoinGecko response")?;

        let token_data = json
            .get(address)
            .and_then(|v| v.as_object())
            .context("Token not found in CoinGecko response")?;

        let price_usd = token_data
            .get("usd")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.6}", v));

        let price_eth = token_data
            .get("eth")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.18}", v));

        Ok(TokenPriceOutput {
            token: address.to_string(),
            token_address: Some(address.to_string()),
            price_usd,
            price_eth,
            last_updated: None,
        })
    }

    async fn fetch_price_by_symbol(
        &self,
        client: &reqwest::Client,
        symbol: &str,
    ) -> Result<TokenPriceOutput> {
        // Map common symbols to CoinGecko IDs
        let coin_id = match symbol {
            "usdc" => "usd-coin",
            "usdt" => "tether",
            "dai" => "dai",
            "weth" => "weth",
            "wbtc" => "wrapped-bitcoin",
            "link" => "chainlink",
            "uni" => "uniswap",
            "aave" => "aave",
            "mkr" => "maker",
            "comp" => "compound-governance-token",
            _ => symbol,
        };

        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd,eth",
            coin_id
        );

        info!("Fetching price by symbol from CoinGecko: {}", url);

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to CoinGecko")?;

        if !response.status().is_success() {
            anyhow::bail!("CoinGecko API returned error: {}", response.status());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse CoinGecko response")?;

        let token_data = json
            .get(coin_id)
            .and_then(|v| v.as_object())
            .context("Token not found in CoinGecko response")?;

        let price_usd = token_data
            .get("usd")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.6}", v));

        let price_eth = token_data
            .get("eth")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.18}", v));

        Ok(TokenPriceOutput {
            token: symbol.to_string(),
            token_address: None,
            price_usd,
            price_eth,
            last_updated: None,
        })
    }

    async fn fetch_eth_price(&self, client: &reqwest::Client) -> Result<TokenPriceOutput> {
        let url = "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd";

        info!("Fetching ETH price from CoinGecko");

        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to send request to CoinGecko")?;

        if !response.status().is_success() {
            anyhow::bail!("CoinGecko API returned error: {}", response.status());
        }

        let json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse CoinGecko response")?;

        let token_data = json
            .get("ethereum")
            .and_then(|v| v.as_object())
            .context("ETH price not found in CoinGecko response")?;

        let price_usd = token_data
            .get("usd")
            .and_then(|v| v.as_f64())
            .map(|v| format!("{:.6}", v));

        Ok(TokenPriceOutput {
            token: "ETH".to_string(),
            token_address: None,
            price_usd,
            price_eth: Some("1.0".to_string()),
            last_updated: None,
        })
    }

    #[instrument(skip(self))]
    async fn get_eth_balance(&self, address: Address) -> Result<BalanceOutput> {
        info!("Querying ETH balance for address: {:?}", address);

        let balance = self
            .provider
            .get_balance(address, None)
            .await
            .context("Failed to query ETH balance")?;

        let eth_balance = Decimal::from_str(&balance.to_string())
            .context("Failed to convert balance to Decimal")?
            / Decimal::from(1_000_000_000_000_000_000u64);

        Ok(BalanceOutput {
            wallet_address: format!("{:?}", address),
            token_address: None,
            balance: format!("{:.18}", eth_balance),
            decimals: 18,
            raw_balance: balance.to_string(),
        })
    }

    fn create_balance_of_function() -> Function {
        Function {
            name: "balanceOf".to_string(),
            inputs: vec![Param {
                name: "owner".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            }],
            outputs: vec![Param {
                name: "".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            }],
            constant: None,
            state_mutability: StateMutability::View,
        }
    }

    fn create_decimals_function() -> Function {
        Function {
            name: "decimals".to_string(),
            inputs: vec![],
            outputs: vec![Param {
                name: "".to_string(),
                kind: ParamType::Uint(8),
                internal_type: None,
            }],
            constant: None,
            state_mutability: StateMutability::View,
        }
    }

    #[instrument(skip(self))]
    async fn get_erc20_balance(
        &self,
        wallet_address: Address,
        token_address_str: String,
    ) -> Result<BalanceOutput> {
        let token_address =
            Address::from_str(&token_address_str).context("Invalid token contract address")?;

        info!(
            "Querying ERC20 balance for wallet: {:?}, token: {:?}",
            wallet_address, token_address
        );

        // Create ERC20 functions
        let balance_of = Self::create_balance_of_function();
        let decimals_fn = Self::create_decimals_function();

        // Call balanceOf
        let balance_input = balance_of
            .encode_input(&[Token::Address(wallet_address)])
            .context("Failed to encode balanceOf call")?;

        let balance_tx = TransactionRequest::new()
            .to(token_address)
            .data(balance_input);
        let balance_call_result = self
            .provider
            .call(&balance_tx.into(), None)
            .await
            .context("Failed to call balanceOf")?;

        // Call decimals
        let decimals_input = decimals_fn
            .encode_input(&[])
            .context("Failed to encode decimals call")?;

        let decimals_tx = TransactionRequest::new()
            .to(token_address)
            .data(decimals_input);
        let decimals_call_result = self
            .provider
            .call(&decimals_tx.into(), None)
            .await
            .context("Failed to call decimals")?;

        // Decode balanceOf result
        let balance_tokens = balance_of
            .decode_output(&balance_call_result)
            .context("Failed to decode balanceOf result")?;
        let balance_token = balance_tokens.first().context("No balance in result")?;

        let balance = match balance_token {
            Token::Uint(val) => *val,
            _ => anyhow::bail!("Unexpected balance token type"),
        };

        // Decode decimals result
        let decimals_tokens = decimals_fn
            .decode_output(&decimals_call_result)
            .context("Failed to decode decimals result")?;
        let decimals_token = decimals_tokens.first().context("No decimals in result")?;

        let decimals = match decimals_token {
            Token::Uint(val) => {
                let d = val.to_string().parse::<u64>()? as u8;
                d
            }
            _ => anyhow::bail!("Unexpected decimals token type"),
        };

        // Convert to decimal with proper precision
        let decimals_u32 = u32::from(decimals);
        let divisor = Decimal::from(10u64.pow(decimals_u32));
        let token_balance = Decimal::from_str(&balance.to_string())
            .context("Failed to convert balance to Decimal")?
            / divisor;

        Ok(BalanceOutput {
            wallet_address: format!("{:?}", wallet_address),
            token_address: Some(token_address_str),
            balance: format!("{:.prec$}", token_balance, prec = decimals_u32 as usize),
            decimals,
            raw_balance: balance.to_string(),
        })
    }
}

#[tool_handler]
impl ServerHandler for EthMcpServer {
    fn get_info(&self) -> ServerInfo {
        let tools = self.tool_router.list_all();
        info!("get_info called, router has {} tools", tools.len());
        for tool in &tools {
            info!(
                "Tool registered: {} - {}",
                tool.name,
                tool.description.as_deref().unwrap_or("")
            );
        }
        ServerInfo {
            instructions: Some(
                "Ethereum MCP server for querying balances and executing token swaps".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
