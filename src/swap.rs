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
    types::{Address, TransactionRequest, U256},
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, instrument, warn};

pub const UNISWAP_V2_ROUTER: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
pub const UNISWAP_V3_ROUTER: &str = "0xE592427A0AEce92De3Edee1F18E0157C05861564";
pub const UNISWAP_V3_QUOTER_V2: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
pub const UNISWAP_V3_QUOTER: &str = "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6"; // Old Quoter as fallback
pub const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UniswapVersion {
    V2,
    V3,
}

impl Default for UniswapVersion {
    fn default() -> Self {
        UniswapVersion::V2
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SwapInput {
    /// Source token address (use "ETH" for native ETH)
    pub from_token: String,
    /// Destination token address (use "ETH" for native ETH)
    pub to_token: String,
    /// Amount to swap (in human-readable format, e.g., "1.0")
    pub amount: String,
    /// Slippage tolerance as percentage (e.g., "0.5" for 0.5%)
    pub slippage_tolerance: String,

    /// Uniswap version to use (V2 or V3). If not specified, defaults to V2
    #[serde(default)]
    pub version: Option<UniswapVersion>,
    /// Pool fee for V3 swaps (500, 3000, or 10000). Required for V3, ignored for V2
    #[serde(default)]
    pub pool_fee: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapOutput {
    /// Source token address
    pub from_token: String,
    /// Destination token address
    pub to_token: String,
    /// Input amount (in human-readable format)
    pub input_amount: String,
    /// Estimated output amount (in human-readable format)
    pub estimated_output: String,
    /// Minimum output amount considering slippage
    pub minimum_output: String,
    /// Slippage tolerance percentage
    pub slippage_tolerance: String,
    /// Estimated gas cost in wei
    pub estimated_gas: String,
    /// Estimated gas cost in ETH
    pub estimated_gas_eth: String,
    /// Price impact percentage (if calculable)
    pub price_impact: Option<String>,
    /// Whether the swap involves ETH
    pub involves_eth: bool,
    /// Uniswap version used for this swap
    pub version: String,
}

pub struct SwapProvider {
    provider: Arc<Provider<Http>>,
}

impl SwapProvider {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self { provider }
    }

    #[instrument(skip(self))]
    pub async fn estimate_swap(&self, input: SwapInput) -> Result<SwapOutput> {
        let version = input.version.unwrap_or(UniswapVersion::V2);
        info!(
            "Estimating swap: {} -> {} using Uniswap {:?}",
            input.from_token, input.to_token, version
        );

        match version {
            UniswapVersion::V2 => self.estimate_swap_v2(input).await,
            UniswapVersion::V3 => self.estimate_swap_v3(input).await,
        }
    }

    #[instrument(skip(self))]
    async fn estimate_swap_v2(&self, input: SwapInput) -> Result<SwapOutput> {
        info!("Using Uniswap V2 for swap estimation");

        let from_token = normalize_token_address(&input.from_token)?;
        let to_token = normalize_token_address(&input.to_token)?;
        let slippage = parse_slippage(&input.slippage_tolerance)?;

        let from_is_eth = input.from_token.to_lowercase() == "eth";
        let to_is_eth = input.to_token.to_lowercase() == "eth";

        let from_token_decimals = if from_is_eth {
            18
        } else {
            let from_token_addr = Address::from_str(&from_token)?;
            self.get_token_decimals(from_token_addr).await.unwrap_or(18)
        };

        let amount = parse_amount(&input.amount, from_token_decimals)?;

        let path = if from_is_eth && !to_is_eth {
            vec![
                Address::from_str(WETH_ADDRESS)?,
                Address::from_str(&to_token)?,
            ]
        } else if !from_is_eth && to_is_eth {
            vec![
                Address::from_str(&from_token)?,
                Address::from_str(WETH_ADDRESS)?,
            ]
        } else if !from_is_eth && !to_is_eth {
            vec![
                Address::from_str(&from_token)?,
                Address::from_str(&to_token)?,
            ]
        } else {
            anyhow::bail!("ETH to ETH swap is not supported");
        };

        let router_address = Address::from_str(UNISWAP_V2_ROUTER)?;

        let expected_output = self
            .get_v2_expected_output(&path, amount)
            .await
            .context("Failed to get expected output from V2")?;

        let amount_out_min = calculate_min_output(expected_output, slippage)?;

        let (swap_fn, call_data, _value) = if from_is_eth && !to_is_eth {
            prepare_v2_swap_exact_eth_for_tokens(&path, amount, amount_out_min, Address::zero())?
        } else if !from_is_eth && to_is_eth {
            prepare_v2_swap_exact_tokens_for_eth(&path, amount, amount_out_min, Address::zero())?
        } else {
            prepare_v2_swap_exact_tokens_for_tokens(&path, amount, amount_out_min, Address::zero())?
        };

        // Use a dummy address for simulation (eth_call doesn't require real balance)
        // Using a well-known address that likely has some balance for better simulation
        let dummy_from_address = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
        
        let mut tx_request = TransactionRequest::new()
            .to(router_address)
            .from(dummy_from_address)
            .data(call_data.clone());

        if from_is_eth {
            tx_request = tx_request.value(amount);
        }

        warn!("Simulating V2 swap: to={:?}, from={:?}, data_len={}, value={:?}", 
            router_address,
            dummy_from_address,
            call_data.len(),
            if from_is_eth { Some(amount) } else { None }
        );

        // Try to simulate the swap, but if it fails (e.g., due to approval or balance checks),
        // fall back to using the expected output from getAmountsOut
        let actual_output = match self
            .provider
            .call(&tx_request.clone().into(), None)
            .await
        {
            Ok(call_result) => {
                match decode_v2_swap_result(&swap_fn, &call_result) {
                    Ok(output) => output,
                    Err(e) => {
                        warn!("Failed to decode V2 swap result: {}, using expected output", e);
                        expected_output
                    }
                }
            }
            Err(e) => {
                warn!("V2 swap simulation call failed: {}, using expected output from getAmountsOut", e);
                // Use the expected output from getAmountsOut as fallback
                expected_output
            }
        };

        // Try to estimate gas, but if it fails (e.g., due to transaction revert),
        // use a reasonable default gas estimate
        let gas_estimate = match self
            .provider
            .estimate_gas(&tx_request.clone().into(), None)
            .await
        {
            Ok(gas) => gas,
            Err(e) => {
                warn!("Failed to estimate gas for V2 swap: {}, using default gas estimate", e);
                // Use default gas estimates for Uniswap V2 swaps
                // V2 swaps typically use 100k-200k gas
                U256::from(150_000u64)
            }
        };

        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .context("Failed to get gas price")?;

        let gas_cost_wei = gas_estimate * gas_price;
        let gas_cost_eth = Decimal::from_str(&gas_cost_wei.to_string())
            .context("Failed to convert gas cost to Decimal")?
            / Decimal::from(1_000_000_000_000_000_000u64);

        let to_token_decimals = if to_is_eth {
            18
        } else {
            self.get_token_decimals(Address::from_str(&to_token)?)
                .await
                .unwrap_or(18)
        };

        let output_decimal = Decimal::from_str(&actual_output.to_string())
            .context("Failed to convert output to Decimal")?
            / Decimal::from(10u64.pow(u32::from(to_token_decimals)));

        let min_output_decimal = Decimal::from_str(&amount_out_min.to_string())
            .context("Failed to convert min output to Decimal")?
            / Decimal::from(10u64.pow(u32::from(to_token_decimals)));

        Ok(SwapOutput {
            from_token: input.from_token,
            to_token: input.to_token,
            input_amount: input.amount,
            estimated_output: format!(
                "{:.prec$}",
                output_decimal,
                prec = to_token_decimals as usize
            ),
            minimum_output: format!(
                "{:.prec$}",
                min_output_decimal,
                prec = to_token_decimals as usize
            ),
            slippage_tolerance: input.slippage_tolerance,
            estimated_gas: gas_estimate.to_string(),
            estimated_gas_eth: format!("{:.18}", gas_cost_eth),
            price_impact: None,
            involves_eth: from_is_eth || to_is_eth,
            version: "V2".to_string(),
        })
    }

    #[instrument(skip(self))]
    async fn estimate_swap_v3(&self, input: SwapInput) -> Result<SwapOutput> {
        info!("Using Uniswap V3 for swap estimation");

        let pool_fee = input.pool_fee.unwrap_or(3000);
        if !matches!(pool_fee, 500 | 3000 | 10000) {
            anyhow::bail!("Pool fee must be 500, 3000, or 10000");
        }

        let from_token = normalize_token_address(&input.from_token)?;
        let to_token = normalize_token_address(&input.to_token)?;
        let slippage = parse_slippage(&input.slippage_tolerance)?;

        let from_is_eth = input.from_token.to_lowercase() == "eth";
        let to_is_eth = input.to_token.to_lowercase() == "eth";

        let from_token_decimals = if from_is_eth {
            18
        } else {
            let from_token_addr = Address::from_str(&from_token)?;
            self.get_token_decimals(from_token_addr).await.unwrap_or(18)
        };

        let amount = parse_amount(&input.amount, from_token_decimals)?;

        // For V3, use actual token addresses (WETH for ETH)
        let token_in = if from_is_eth {
            Address::from_str(WETH_ADDRESS)?
        } else {
            Address::from_str(&from_token)?
        };
        
        let token_out = if to_is_eth {
            Address::from_str(WETH_ADDRESS)?
        } else {
            Address::from_str(&to_token)?
        };

        let router_address = Address::from_str(UNISWAP_V3_ROUTER)?;

        let expected_output = self
            .get_v3_expected_output(token_in, token_out, pool_fee, amount, false)
            .await
            .context("Failed to get expected output from V3")?;

        let amount_out_min = calculate_min_output(expected_output, slippage)?;

        let (swap_fn, call_data, _value) = if from_is_eth && !to_is_eth {
            prepare_v3_exact_input_single_native(
                token_out,
                pool_fee,
                amount,
                amount_out_min,
                Address::zero(),
            )?
        } else if !from_is_eth && to_is_eth {
            prepare_v3_exact_input_single(
                token_in,
                token_out,
                pool_fee,
                amount,
                amount_out_min,
                Address::zero(),
            )?
        } else {
            prepare_v3_exact_input_single(
                token_in,
                token_out,
                pool_fee,
                amount,
                amount_out_min,
                Address::zero(),
            )?
        };

        // Use a dummy address for simulation (eth_call doesn't require real balance)
        let dummy_from_address = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
        
        let mut tx_request = TransactionRequest::new()
            .to(router_address)
            .from(dummy_from_address)
            .data(call_data.clone());

        if from_is_eth {
            tx_request = tx_request.value(amount);
        }

        warn!("Simulating V3 swap: to={:?}, from={:?}, data_len={}, value={:?}", 
            router_address,
            dummy_from_address,
            call_data.len(),
            if from_is_eth { Some(amount) } else { None }
        );
        
        // Try to simulate the swap, but if it fails (e.g., due to approval or balance checks),
        // fall back to using the expected output from quoteExactInputSingle
        let actual_output = match self
            .provider
            .call(&tx_request.clone().into(), None)
            .await
        {
            Ok(call_result) => {
                match decode_v3_swap_result(&swap_fn, &call_result) {
                    Ok(output) => output,
                    Err(e) => {
                        warn!("Failed to decode V3 swap result: {}, using expected output", e);
                        expected_output
                    }
                }
            }
            Err(e) => {
                warn!("V3 swap simulation call failed: {}, using expected output from quoteExactInputSingle", e);
                // Use the expected output from quoteExactInputSingle as fallback
                expected_output
            }
        };

        // Try to estimate gas, but if it fails (e.g., due to transaction revert),
        // use a reasonable default gas estimate
        let gas_estimate = match self
            .provider
            .estimate_gas(&tx_request.clone().into(), None)
            .await
        {
            Ok(gas) => gas,
            Err(e) => {
                warn!("Failed to estimate gas for V3 swap: {}, using default gas estimate", e);
                // Use default gas estimates for Uniswap V3 swaps
                // V3 swaps typically use 150k-250k gas
                U256::from(200_000u64)
            }
        };

        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .context("Failed to get gas price")?;

        let gas_cost_wei = gas_estimate * gas_price;
        let gas_cost_eth = Decimal::from_str(&gas_cost_wei.to_string())
            .context("Failed to convert gas cost to Decimal")?
            / Decimal::from(1_000_000_000_000_000_000u64);

        let to_token_decimals = if to_is_eth {
            18
        } else {
            self.get_token_decimals(token_out).await.unwrap_or(18)
        };

        let output_decimal = Decimal::from_str(&actual_output.to_string())
            .context("Failed to convert output to Decimal")?
            / Decimal::from(10u64.pow(u32::from(to_token_decimals)));

        let min_output_decimal = Decimal::from_str(&amount_out_min.to_string())
            .context("Failed to convert min output to Decimal")?
            / Decimal::from(10u64.pow(u32::from(to_token_decimals)));

        Ok(SwapOutput {
            from_token: input.from_token,
            to_token: input.to_token,
            input_amount: input.amount,
            estimated_output: format!(
                "{:.prec$}",
                output_decimal,
                prec = to_token_decimals as usize
            ),
            minimum_output: format!(
                "{:.prec$}",
                min_output_decimal,
                prec = to_token_decimals as usize
            ),
            slippage_tolerance: input.slippage_tolerance,
            estimated_gas: gas_estimate.to_string(),
            estimated_gas_eth: format!("{:.18}", gas_cost_eth),
            price_impact: None,
            involves_eth: from_is_eth || to_is_eth,
            version: "V3".to_string(),
        })
    }

    /// Query Uniswap V2 Router to get expected output amount for a given input amount and swap path.
    ///
    /// This function calls the `getAmountsOut` function on the Uniswap V2 Router contract.
    /// It performs a read-only query (using `eth_call`) to calculate the expected output amount
    /// based on current pool reserves and prices, without executing an actual swap transaction.
    ///
    /// # Function Flow
    /// 1. Constructs the `getAmountsOut` function signature with parameters:
    ///    - `amountIn`: The input amount (in token's smallest unit, e.g., wei for ETH)
    ///    - `path`: Array of token addresses representing the swap path
    ///      - Example: [WETH, USDC] for ETH -> USDC swap
    ///      - Example: [USDC, WETH] for USDC -> ETH swap
    /// 2. Encodes the function call data using ABI encoding
    /// 3. Sends a read-only `eth_call` to the Uniswap V2 Router contract
    /// 4. Decodes the response which returns an array of amounts at each step
    /// 5. Extracts the last element (final output amount) from the amounts array
    ///
    /// # Parameters
    /// - `path`: The swap path as an array of token addresses. The first element is the input token,
    ///   and the last element is the output token. For direct swaps, the path has 2 elements.
    ///   For multi-hop swaps, it can have more elements.
    /// - `amount_in`: The input amount in the smallest unit of the input token (e.g., wei for ETH,
    ///   or raw units for ERC20 tokens)
    ///
    /// # Returns
    /// - `Ok(U256)`: The expected output amount in the smallest unit of the output token
    /// - `Err`: If the function call fails, encoding/decoding fails, or the response format is unexpected
    ///
    /// # Example
    /// ```
    /// // For ETH -> USDC swap with 0.1 ETH input:
    /// // path = [WETH_ADDRESS, USDC_ADDRESS]
    /// // amount_in = 100000000000000000 (0.1 ETH in wei)
    /// // Returns: expected USDC amount in raw units (6 decimals)
    /// ```
    ///
    /// # Notes
    /// - This is a read-only operation that does not modify blockchain state
    /// - The result is based on current pool reserves and may change between calls
    /// - No gas is consumed for this query (read-only `eth_call`)
    async fn get_v2_expected_output(&self, path: &[Address], amount_in: U256) -> Result<U256> {
        let router_address = Address::from_str(UNISWAP_V2_ROUTER)?;

        let get_amounts_out_fn = Function {
            name: "getAmountsOut".to_string(),
            inputs: vec![
                Param {
                    name: "amountIn".to_string(),
                    kind: ParamType::Uint(256),
                    internal_type: None,
                },
                Param {
                    name: "path".to_string(),
                    kind: ParamType::Array(Box::new(ParamType::Address)),
                    internal_type: None,
                },
            ],
            outputs: vec![Param {
                name: "amounts".to_string(),
                kind: ParamType::Array(Box::new(ParamType::Uint(256))),
                internal_type: None,
            }],
            constant: None,
            state_mutability: StateMutability::View,
        };

        let path_tokens: Vec<Token> = path.iter().map(|&addr| Token::Address(addr)).collect();
        let input_data = get_amounts_out_fn
            .encode_input(&[Token::Uint(amount_in), Token::Array(path_tokens)])
            .context("Failed to encode getAmountsOut call")?;

        let tx_request = TransactionRequest::new()
            .to(router_address)
            .data(input_data);

        let result = self
            .provider
            .call(&tx_request.into(), None)
            .await
            .context("Failed to call getAmountsOut")?;

        let decoded = get_amounts_out_fn
            .decode_output(&result)
            .context("Failed to decode getAmountsOut result")?;

        let amounts = match decoded.first() {
            Some(Token::Array(arr)) => arr,
            _ => anyhow::bail!("Unexpected getAmountsOut result format"),
        };

        match amounts.last() {
            Some(Token::Uint(val)) => Ok(*val),
            _ => anyhow::bail!("Failed to extract output amount"),
        }
    }

    async fn get_v3_expected_output(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: U256,
        is_eth: bool,
    ) -> Result<U256> {
        // Try the specified fee first, then fallback to common fees if pool doesn't exist
        let fees_to_try = vec![fee, 3000, 500, 10000];
        
        let mut last_error = None;
        for &try_fee in &fees_to_try {
            match self
                .try_get_v3_expected_output_quoter_v2(token_in, token_out, try_fee, amount_in, is_eth)
                .await
            {
                Ok(result) => {
                    if try_fee != fee {
                        warn!("Pool with fee {} not found, using fee {} instead", fee, try_fee);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    warn!("Failed to get quote with QuoterV2 fee {}: {}", try_fee, e);
                    last_error = Some(e);
                    // Continue trying other fees
                }
            }
        }
        
        // If QuoterV2 fails, try old Quoter as fallback
        warn!("QuoterV2 failed for all fees, trying old Quoter as fallback");
        for &try_fee in &fees_to_try {
            match self
                .try_get_v3_expected_output_quoter(token_in, token_out, try_fee, amount_in, is_eth)
                .await
            {
                Ok(result) => {
                    warn!("Using old Quoter result for fee {}", try_fee);
                    return Ok(result);
                }
                Err(e) => {
                    warn!("Failed to get quote with old Quoter fee {}: {}", try_fee, e);
                    // Continue trying
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Failed to get V3 quote with any fee tier or quoter (tried: {:?})", fees_to_try)
        }))
    }

    async fn try_get_v3_expected_output_quoter_v2(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: U256,
        is_eth: bool,
    ) -> Result<U256> {
        let quoter_address = Address::from_str(UNISWAP_V3_QUOTER_V2)?;

        let quote_exact_input_single_fn = Function {
            name: "quoteExactInputSingle".to_string(),
            inputs: vec![Param {
                name: "params".to_string(),
                kind: ParamType::Tuple(vec![
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Uint(24),
                    ParamType::Uint(256),
                    ParamType::Uint(160),
                ]),
                internal_type: None,
            }],
            outputs: vec![
                Param {
                    name: "amountOut".to_string(),
                    kind: ParamType::Uint(256),
                    internal_type: None,
                },
                Param {
                    name: "sqrtPriceX96After".to_string(),
                    kind: ParamType::Uint(160),
                    internal_type: None,
                },
                Param {
                    name: "initializedTicksCrossed".to_string(),
                    kind: ParamType::Uint(32),
                    internal_type: None,
                },
                Param {
                    name: "gasEstimate".to_string(),
                    kind: ParamType::Uint(32),
                    internal_type: None,
                },
            ],
            constant: None,
            state_mutability: StateMutability::View,
        };

        // token_in is already WETH if it was ETH, so we don't need to convert again
        let actual_token_in = token_in;

        let sqrt_price_limit_x96 = U256::zero();
        let params_tokens = vec![
            Token::Address(actual_token_in),
            Token::Address(token_out),
            Token::Uint(U256::from(fee)),
            Token::Uint(amount_in),
            Token::Uint(sqrt_price_limit_x96),
        ];

        let input_data = quote_exact_input_single_fn
            .encode_input(&[Token::Tuple(params_tokens)])
            .context("Failed to encode quoteExactInputSingle call")?;

        // Use a dummy address for simulation (eth_call doesn't require real balance)
        let dummy_from_address = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
        
        let tx_request = TransactionRequest::new()
            .to(quoter_address)
            .from(dummy_from_address)
            .data(input_data);

        warn!("Calling V3 QuoterV2: token_in={:?}, token_out={:?}, fee={}, amount_in={}", 
            actual_token_in, token_out, fee, amount_in);

        let result = self
            .provider
            .call(&tx_request.into(), None)
            .await
            .map_err(|e| {
                anyhow::anyhow!("QuoterV2 call failed for fee {}: {}", fee, e)
            })?;

        let decoded = quote_exact_input_single_fn
            .decode_output(&result)
            .context("Failed to decode quoteExactInputSingle result")?;

        match decoded.first() {
            Some(Token::Uint(val)) => Ok(*val),
            _ => anyhow::bail!("Failed to extract output amount from V3 quote"),
        }
    }

    async fn try_get_v3_expected_output_quoter(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: U256,
        _is_eth: bool,
    ) -> Result<U256> {
        let quoter_address = Address::from_str(UNISWAP_V3_QUOTER)?;

        // Old Quoter has different interface - returns uint256 directly
        let quote_exact_input_single_fn = Function {
            name: "quoteExactInputSingle".to_string(),
            inputs: vec![
                Param {
                    name: "tokenIn".to_string(),
                    kind: ParamType::Address,
                    internal_type: None,
                },
                Param {
                    name: "tokenOut".to_string(),
                    kind: ParamType::Address,
                    internal_type: None,
                },
                Param {
                    name: "fee".to_string(),
                    kind: ParamType::Uint(24),
                    internal_type: None,
                },
                Param {
                    name: "amountIn".to_string(),
                    kind: ParamType::Uint(256),
                    internal_type: None,
                },
                Param {
                    name: "sqrtPriceLimitX96".to_string(),
                    kind: ParamType::Uint(160),
                    internal_type: None,
                },
            ],
            outputs: vec![Param {
                name: "amountOut".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            }],
            constant: None,
            state_mutability: StateMutability::View,
        };

        let sqrt_price_limit_x96 = U256::zero();
        let input_data = quote_exact_input_single_fn
            .encode_input(&[
                Token::Address(token_in),
                Token::Address(token_out),
                Token::Uint(U256::from(fee)),
                Token::Uint(amount_in),
                Token::Uint(sqrt_price_limit_x96),
            ])
            .context("Failed to encode quoteExactInputSingle call for old Quoter")?;

        let dummy_from_address = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;
        
        let tx_request = TransactionRequest::new()
            .to(quoter_address)
            .from(dummy_from_address)
            .data(input_data);

        warn!("Calling old V3 Quoter: token_in={:?}, token_out={:?}, fee={}, amount_in={}", 
            token_in, token_out, fee, amount_in);

        let result = self
            .provider
            .call(&tx_request.into(), None)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Old Quoter call failed for fee {}: {}", fee, e)
            })?;

        let decoded = quote_exact_input_single_fn
            .decode_output(&result)
            .context("Failed to decode old Quoter result")?;

        match decoded.first() {
            Some(Token::Uint(val)) => Ok(*val),
            _ => anyhow::bail!("Failed to extract output amount from old Quoter"),
        }
    }

    async fn get_token_decimals(&self, token_address: Address) -> Result<u8> {
        let decimals_fn = Function {
            name: "decimals".to_string(),
            inputs: vec![],
            outputs: vec![Param {
                name: "".to_string(),
                kind: ParamType::Uint(8),
                internal_type: None,
            }],
            constant: None,
            state_mutability: StateMutability::View,
        };

        let input_data = decimals_fn
            .encode_input(&[])
            .context("Failed to encode decimals call")?;

        let tx_request = TransactionRequest::new().to(token_address).data(input_data);

        let result = self
            .provider
            .call(&tx_request.into(), None)
            .await
            .context("Failed to call decimals")?;

        let decoded = decimals_fn
            .decode_output(&result)
            .context("Failed to decode decimals result")?;

        match decoded.first() {
            Some(Token::Uint(val)) => {
                let d = val.to_string().parse::<u64>()? as u8;
                Ok(d)
            }
            _ => anyhow::bail!("Unexpected decimals result format"),
        }
    }
}

fn normalize_token_address(token: &str) -> Result<String> {
    let token_lower = token.to_lowercase();
    if token_lower == "eth" || token_lower == "ethereum" {
        Ok(WETH_ADDRESS.to_string())
    } else if token_lower.starts_with("0x") && token_lower.len() == 42 {
        Ok(token_lower)
    } else {
        anyhow::bail!("Invalid token address or symbol: {}", token)
    }
}

fn parse_amount(amount_str: &str, decimals: u8) -> Result<U256> {
    let amount_decimal = Decimal::from_str(amount_str).context("Failed to parse amount")?;
    let divisor = Decimal::from(10u64.pow(u32::from(decimals)));
    let amount_units = amount_decimal * divisor;
    
    // Convert Decimal to string without decimal point
    // Truncate to get integer part only, then convert to string
    let amount_units_truncated = amount_units.trunc();
    let amount_units_str = amount_units_truncated.to_string();
    
    // Remove any decimal point that might still be present (e.g., "100.0")
    let amount_units_str = if amount_units_str.contains('.') {
        amount_units_str.split('.').next().unwrap_or(&amount_units_str)
    } else {
        &amount_units_str
    };
    
    let amount_u256 = U256::from_dec_str(amount_units_str)
        .context("Failed to convert amount to U256")?;
    Ok(amount_u256)
}

fn parse_slippage(slippage_str: &str) -> Result<Decimal> {
    let slippage = Decimal::from_str(slippage_str).context("Failed to parse slippage tolerance")?;
    if slippage < Decimal::ZERO || slippage > Decimal::from(100) {
        anyhow::bail!("Slippage tolerance must be between 0 and 100");
    }
    Ok(slippage)
}

fn calculate_min_output(output: U256, slippage: Decimal) -> Result<U256> {
    let slippage_decimal = slippage / Decimal::from(100);
    let one_minus_slippage = Decimal::from(1) - slippage_decimal;
    let min_output_decimal = Decimal::from_str(&output.to_string())? * one_minus_slippage;
    
    // Convert Decimal to string without decimal point
    let min_output_truncated = min_output_decimal.trunc();
    let min_output_str = min_output_truncated.to_string();
    
    // Remove any decimal point that might still be present
    let min_output_str = if min_output_str.contains('.') {
        min_output_str.split('.').next().unwrap_or(&min_output_str)
    } else {
        &min_output_str
    };
    
    let min_output = U256::from_dec_str(min_output_str)
        .context("Failed to convert min output to U256")?;
    Ok(min_output)
}

fn prepare_v2_swap_exact_eth_for_tokens(
    path: &[Address],
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
) -> Result<(Function, Bytes, U256)> {
    let function = Function {
        name: "swapExactETHForTokens".to_string(),
        inputs: vec![
            Param {
                name: "amountOutMin".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "path".to_string(),
                kind: ParamType::Array(Box::new(ParamType::Address)),
                internal_type: None,
            },
            Param {
                name: "to".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "deadline".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![Param {
            name: "amounts".to_string(),
            kind: ParamType::Array(Box::new(ParamType::Uint(256))),
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::Payable,
    };

    let deadline = U256::from(u64::MAX);
    let path_tokens: Vec<Token> = path.iter().map(|&addr| Token::Address(addr)).collect();
    let data = function
        .encode_input(&[
            Token::Uint(amount_out_min),
            Token::Array(path_tokens),
            Token::Address(to),
            Token::Uint(deadline),
        ])
        .context("Failed to encode swapExactETHForTokens")?;

    Ok((function, data.into(), amount_in))
}

fn prepare_v2_swap_exact_tokens_for_eth(
    path: &[Address],
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
) -> Result<(Function, Bytes, U256)> {
    let function = Function {
        name: "swapExactTokensForETH".to_string(),
        inputs: vec![
            Param {
                name: "amountIn".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "amountOutMin".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "path".to_string(),
                kind: ParamType::Array(Box::new(ParamType::Address)),
                internal_type: None,
            },
            Param {
                name: "to".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "deadline".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![Param {
            name: "amounts".to_string(),
            kind: ParamType::Array(Box::new(ParamType::Uint(256))),
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    };

    let deadline = U256::from(u64::MAX);
    let path_tokens: Vec<Token> = path.iter().map(|&addr| Token::Address(addr)).collect();
    let data = function
        .encode_input(&[
            Token::Uint(amount_in),
            Token::Uint(amount_out_min),
            Token::Array(path_tokens),
            Token::Address(to),
            Token::Uint(deadline),
        ])
        .context("Failed to encode swapExactTokensForETH")?;

    Ok((function, data.into(), U256::zero()))
}

fn prepare_v2_swap_exact_tokens_for_tokens(
    path: &[Address],
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
) -> Result<(Function, Bytes, U256)> {
    let function = Function {
        name: "swapExactTokensForTokens".to_string(),
        inputs: vec![
            Param {
                name: "amountIn".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "amountOutMin".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "path".to_string(),
                kind: ParamType::Array(Box::new(ParamType::Address)),
                internal_type: None,
            },
            Param {
                name: "to".to_string(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "deadline".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        outputs: vec![Param {
            name: "amounts".to_string(),
            kind: ParamType::Array(Box::new(ParamType::Uint(256))),
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::NonPayable,
    };

    let deadline = U256::from(u64::MAX);
    let path_tokens: Vec<Token> = path.iter().map(|&addr| Token::Address(addr)).collect();
    let data = function
        .encode_input(&[
            Token::Uint(amount_in),
            Token::Uint(amount_out_min),
            Token::Array(path_tokens),
            Token::Address(to),
            Token::Uint(deadline),
        ])
        .context("Failed to encode swapExactTokensForTokens")?;

    Ok((function, data.into(), U256::zero()))
}

fn decode_v2_swap_result(function: &Function, result: &Bytes) -> Result<U256> {
    let decoded = function
        .decode_output(result)
        .context("Failed to decode V2 swap result")?;

    let amounts = match decoded.first() {
        Some(Token::Array(arr)) => arr,
        _ => anyhow::bail!("Unexpected V2 swap result format"),
    };

    match amounts.last() {
        Some(Token::Uint(val)) => Ok(*val),
        _ => anyhow::bail!("Failed to extract output amount from V2 swap result"),
    }
}

fn prepare_v3_exact_input_single(
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
    amount_out_min: U256,
    recipient: Address,
) -> Result<(Function, Bytes, U256)> {
    let function = Function {
        name: "exactInputSingle".to_string(),
        inputs: vec![Param {
            name: "params".to_string(),
            kind: ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(160),
            ]),
            internal_type: None,
        }],
        outputs: vec![Param {
            name: "amountOut".to_string(),
            kind: ParamType::Uint(256),
            internal_type: None,
        }],
        constant: None,
        state_mutability: StateMutability::Payable,
    };

    let deadline = U256::from(u64::MAX);
    let sqrt_price_limit_x96 = U256::zero();

    let params_tokens = vec![
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(U256::from(fee)),
        Token::Address(recipient),
        Token::Uint(deadline),
        Token::Uint(amount_in),
        Token::Uint(amount_out_min),
        Token::Uint(sqrt_price_limit_x96),
    ];

    let data = function
        .encode_input(&[Token::Tuple(params_tokens)])
        .context("Failed to encode exactInputSingle")?;

    Ok((function, data.into(), U256::zero()))
}

fn prepare_v3_exact_input_single_native(
    token_out: Address,
    fee: u32,
    amount_in: U256,
    amount_out_min: U256,
    recipient: Address,
) -> Result<(Function, Bytes, U256)> {
    let weth_address = Address::from_str(WETH_ADDRESS)?;
    prepare_v3_exact_input_single(
        weth_address,
        token_out,
        fee,
        amount_in,
        amount_out_min,
        recipient,
    )
}

fn decode_v3_swap_result(function: &Function, result: &Bytes) -> Result<U256> {
    let decoded = function
        .decode_output(result)
        .context("Failed to decode V3 swap result")?;

    match decoded.first() {
        Some(Token::Uint(val)) => Ok(*val),
        _ => anyhow::bail!("Failed to extract output amount from V3 swap result"),
    }
}
