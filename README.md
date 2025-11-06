# Ethereum Trading MCP Server

<div align="center">

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tokio](https://img.shields.io/badge/Tokio-000000?style=flat&logo=rust&logoColor=white)](https://tokio.rs/)
[![Ethereum](https://img.shields.io/badge/Ethereum-3C3C3D?style=flat&logo=ethereum&logoColor=white)](https://ethereum.org/)
[![Ethers.rs](https://img.shields.io/badge/Ethers.rs-627EEA?style=flat&logo=ethereum&logoColor=white)](https://github.com/gakonst/ethers-rs)
[![MCP](https://img.shields.io/badge/MCP-Model%20Context%20Protocol-000000?style=flat)](https://modelcontextprotocol.io/)
[![Uniswap](https://img.shields.io/badge/Uniswap-FF007A?style=flat&logo=uniswap&logoColor=white)](https://uniswap.org/)

</div>

A Model Context Protocol (MCP) server implemented in Rust that enables AI agents to query Ethereum balances and simulate token swaps on Uniswap.

**[Quick Start Guide](docs/quick-start.md)** - Get up and running quickly.

## Features

The server provides three MCP tools:

1. **`get_balance`** - Query ETH and ERC20 token balances

   - Input: wallet address, optional token contract address
   - Output: balance information with proper decimals

2. **`get_token_price`** - Get current token price in USD and ETH

   - Input: token address (0x...) or symbol (e.g., "USDC", "WETH")
   - Output: price data from CoinGecko API

3. **`swap_tokens`** - Simulate token swaps on Uniswap V2 or V3
   - Input: from_token, to_token, amount, slippage tolerance
   - Output: simulation result showing estimated output and gas costs
   - **Note**: Constructs real Uniswap transactions and simulates them using `eth_call` without executing on-chain

## Project Structure

```
src/
├── main.rs          # Server entry point and transport configuration
├── server.rs        # MCP server implementation with tool handlers
├── swap.rs          # Uniswap V2/V3 swap simulation logic
└── bin/
    ├── client.rs               # MCP client for testing
    ├── test_eth_balance.rs     # Test for ETH balance queries
    ├── test_erc20_balance.rs   # Test for ERC20 balance queries
    ├── test_token_price.rs     # Test for token price queries
    └── test_swap.rs            # Test for swap simulations
```

## Dependencies

- **Rust** (latest stable version)
- **tokio** - Async runtime
- **ethers-rs** - Ethereum RPC client library
- **rmcp** - MCP SDK for Rust
- **rust_decimal** - Financial precision handling
- **tracing** - Structured logging
- **reqwest** - HTTP client for price API calls
- **ethabi** - ABI encoding/decoding

## Setup Instructions

### Prerequisites

1. Install Rust (if not already installed):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone the repository:
   ```bash
   git clone https://github.com/chenjjiaa/eth-mcp;
   cd eth-mcp
   ```

### Environment Variables

Create a `.env` file in the project root (or use environment variables):

```bash
# Ethereum RPC endpoint (required)
ETH_RPC_URL=https://eth.llamarpc.com
# Or use Infura/Alchemy:
# ETH_RPC_URL=https://mainnet.infura.io/v3/YOUR_API_KEY

# Server configuration (optional)
SERVER_HOST=127.0.0.1        # Default: 127.0.0.1
SERVER_PORT=0                # Default: 0 (stdio mode). Set to >0 for TCP mode

# Logging (optional)
RUST_LOG=info                 # Default: info. Options: trace, debug, info, warn, error

# Wallet address
WALLET_ADDRESS=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045
# USDC contract addr
TOKEN_ADDRESS=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
```

**Note**: The server defaults to stdio mode (MCP standard) when `SERVER_PORT=0`. For TCP mode, set `SERVER_PORT` to a valid port number.

### Build and Run

1. **Build the project**:

   ```bash
   cargo b
   # Or for release:
   cargo b --release
   ```

2. **Run the server**:

   ```bash
   # Stdio mode (default, for MCP clients)
   cargo r --bin server

   # Or directly:
   ./target/debug/server
   ```

3. **Run tests**:

   ```bash
   # Run all tests
   cargo test

   # Run specific test binaries
   cargo r --bin test_eth_balance
   cargo r --bin test_erc20_balance
   cargo r --bin test_token_price
   cargo r --bin test_swap
   ```

## Example MCP Tool Calls

### 1. Get ETH Balance

**Request**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_balance",
    "arguments": {
      "wallet_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
    }
  }
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\n  \"wallet_address\": \"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb\",\n  \"token_address\": null,\n  \"balance\": \"0.000000000000000000\",\n  \"decimals\": 18,\n  \"raw_balance\": \"0\"\n}"
      }
    ]
  }
}
```

### 2. Get ERC20 Token Balance

**Request**:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "get_balance",
    "arguments": {
      "wallet_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
      "token_address": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
    }
  }
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\n  \"wallet_address\": \"0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb\",\n  \"token_address\": \"0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48\",\n  \"balance\": \"1000.000000\",\n  \"decimals\": 6,\n  \"raw_balance\": \"1000000000\"\n}"
      }
    ]
  }
}
```

### 3. Get Token Price

**Request**:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_token_price",
    "arguments": {
      "token": "USDC"
    }
  }
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\n  \"token\": \"USDC\",\n  \"token_address\": null,\n  \"price_usd\": \"1.000000\",\n  \"price_eth\": \"0.000300\",\n  \"last_updated\": null\n}"
      }
    ]
  }
}
```

### 4. Simulate Token Swap

**Request**:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "swap_tokens",
    "arguments": {
      "from_token": "ETH",
      "to_token": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      "amount": "1.0",
      "slippage_tolerance": "0.5",
      "version": "v2"
    }
  }
}
```

**Response**:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\n  \"from_token\": \"ETH\",\n  \"to_token\": \"0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48\",\n  \"input_amount\": \"1.0\",\n  \"estimated_output\": \"3200.123456\",\n  \"minimum_output\": \"3184.122842\",\n  \"slippage_tolerance\": \"0.5\",\n  \"estimated_gas\": \"150000\",\n  \"estimated_gas_eth\": \"0.003000\",\n  \"price_impact\": null,\n  \"involves_eth\": true,\n  \"version\": \"v2\"\n}"
      }
    ]
  }
}
```

## Design Decisions

1. **MCP Protocol Implementation**: Used the `rmcp` SDK to handle MCP protocol communication, which provides type-safe tool registration and request/response handling. The server supports both stdio (standard MCP mode) and TCP transport for flexibility.

2. **Ethereum Interaction**: Leveraged `ethers-rs` for Ethereum RPC interactions, using direct `eth_call` for ERC20 balance queries and swap simulations. This approach ensures we're working with real on-chain data without requiring transaction signing for read operations.

3. **Financial Precision**: Used `rust_decimal` throughout the codebase for all financial calculations to avoid floating-point precision errors. All token amounts are handled with proper decimal places based on the token's decimals field.

4. **Swap Simulation**: For swap operations, the implementation constructs real Uniswap V2/V3 transactions and simulates them using `eth_call` (via `provider.call()`), which executes the transaction locally without broadcasting it to the network. This provides accurate estimates including gas costs without requiring actual transaction execution.

5. **Price Data**: Integrated CoinGecko API for token price data, supporting both contract addresses and symbol lookups. The implementation handles common token symbols and provides fallback mechanisms for price queries.

## Known Limitations and Assumptions

1. **Ethereum Mainnet Only**: The server is currently configured for Ethereum mainnet. Support for testnets (Goerli, Sepolia) would require additional configuration and potentially different Uniswap router addresses.

2. **Price API Dependency**: Token price queries depend on CoinGecko's free API, which has rate limits. High-frequency queries may hit rate limits, and the API may not support all tokens.

3. **Uniswap Router Addresses**: The implementation uses hardcoded Uniswap V2 and V3 router addresses for mainnet. Different networks or future router upgrades would require code changes.

4. **Gas Estimation**: Gas estimates are approximate and based on transaction simulation. Actual gas costs may vary depending on network conditions at execution time.

5. **No Wallet Management or Transaction Signing**: The server only simulates swaps and does not execute them on-chain. Wallet management, private key handling, and transaction signing functionality are not implemented. The server cannot sign or broadcast transactions to the blockchain.

6. **Slippage Calculation**: Minimum output calculation uses simple percentage-based slippage. More sophisticated slippage models (e.g., dynamic slippage based on pool liquidity) are not implemented.

7. **Price Impact**: Price impact calculation is not fully implemented for all swap scenarios, especially for large trades that might affect pool prices.

## TODO / Future Improvements

[Future Improvements](docs/future-improvements.md)

## Changelog

[Changelog](docs/CHANGELOG.md)

## License

This project is licensed under the [Apache License 2.0](LICENSE).
