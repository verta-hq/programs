# Verta Solana Programs

This repository contains the Solana programs powering the Verta platform, built using the [Anchor Framework](https://www.anchor-lang.com/).

## 🏗️ Architecture

This repository follows the standard Anchor workspace structure:

- **`programs/`**: Contains the Rust source code for all of our Solana programs. This is where the core logic for royalties and the launchpad resides.
- **`tests/`**: Contains the TypeScript integration tests that run against a local validator to ensure all on-chain logic functions as expected before deployment.
- **`migrations/`**: Contains deployment scripts.

## 🚀 Key Features

- **Automated Royalty Splits**: Programmable revenue distribution for creators, investors, and the platform on every episode unlock.
- **Launchpad Mechanics**: Secure, trustless escrow and funding mechanics for creators looking to fund their next micro-series.
- **Token-Gating Validation**: Verifying NFT and token ownership for premium content access.

## 🛠️ Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation)
- Node.js & npm

### Installation

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Build the programs:**
   ```bash
   anchor build
   ```

## 🧪 Testing

To run the integration tests against a local Solana Test Validator:

```bash
anchor test
```
