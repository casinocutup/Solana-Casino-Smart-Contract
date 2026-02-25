# Solana Casino Smart Contract

**Provably Fair On-Chain Casino Games (Dice & Plinko — Solcasino Fork)**

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## 📋 Description

This is a production-grade, open-source Anchor framework smart contract for Solana, implementing provably fair casino games inspired by Solcasino. The contract features:

- **Dice Game**: Players bet SOL or SPL tokens, choose to roll under/over a target number (1-100), and receive payouts based on probability with configurable house edge.
- **Plinko Game**: Players bet and simulate a ball drop through pegs (8-16 rows), with VRF-determined paths and multipliers ranging from 0.2x to 1000x.
- **Provably Fair**: Uses VRF (Verifiable Random Function) oracles (Switchboard VRF compatible) for cryptographically verifiable randomness.
- **Configurable House Edge**: Admin-configurable house edge (1-5%) and bet limits.
- **Multi-Token Support**: Native SOL and SPL tokens (e.g., USDC).

## ✨ Features

- ✅ **Provably Fair**: VRF-based randomness for verifiable game outcomes
- ✅ **House Edge**: Configurable house edge (1-5% in basis points)
- ✅ **Bet Limits**: Minimum and maximum bet amounts
- ✅ **SOL & SPL Support**: Native SOL and SPL token betting
- ✅ **Security**: Reentrancy guards, overflow checks, access controls
- ✅ **Modular Design**: Clean separation of game logic (Dice, Plinko, VRF)
- ✅ **Comprehensive Tests**: Full test coverage for all game scenarios
- ✅ **Production Ready**: Error handling, custom errors, PDA-based accounts

## 🛠️ Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools) (v1.18.0+)
- [Anchor Framework](https://www.anchor-lang.com/docs/installation) (v0.30.1+)
- [Node.js](https://nodejs.org/) (v18+) and [Yarn](https://yarnpkg.com/) or npm

### Setup

1. **Install Anchor CLI:**
   ```bash
   cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked
   ```

2. **Clone and build:**
   ```bash
   git clone <repository-url>
   cd solana-casino-smart-contract
   anchor build
   ```

3. **Install dependencies:**
   ```bash
   yarn install
   # or
   npm install
   ```

4. **Verify installation:**
   ```bash
   anchor --version
   solana --version
   ```

## ⚙️ Configuration

### Anchor.toml

Edit `Anchor.toml` to configure your deployment:

```toml
[provider]
cluster = "Devnet"  # or "Mainnet", "Localnet"
wallet = "~/.config/solana/id.json"

[programs.devnet]
solana_casino_smart_contract = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
```

### VRF Oracle Setup

This contract is designed to work with Switchboard VRF or compatible VRF oracles. To integrate:

1. **Switchboard VRF**: Set up a Switchboard VRF account and pass the request account to the resolve instructions.
2. **Alternative VRF**: Modify the `vrf.rs` module to integrate with ORAO, Chainlink, or other VRF providers.

**Note**: The current implementation includes VRF helper functions that can work with any VRF provider that returns random bytes.

## 🚀 Usage

### Build

```bash
anchor build
```

### Deploy

**Deploy to Devnet:**
```bash
anchor deploy --provider.cluster devnet
```

**Deploy to Mainnet:**
```bash
anchor deploy --provider.cluster mainnet
```

### Test

Run the test suite:
```bash
anchor test
```

Or run tests directly:
```bash
yarn test
```

### Program Instructions

#### Initialize Casino

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

const program = anchor.workspace.SolanaCasinoSmartContract;

// Initialize with 2% house edge, min bet 0.0001 SOL, max bet 10 SOL
await program.methods
  .initializeCasino(
    200,                    // house_edge_bps (200 = 2%)
    new anchor.BN(100000),  // min_bet (lamports)
    new anchor.BN(10 * anchor.web3.LAMPORTS_PER_SOL) // max_bet
  )
  .accounts({
    vault: vaultPda,
    admin: admin.publicKey,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

#### Place Dice Bet (SOL)

```typescript
// Place bet: roll under 50, bet amount 0.1 SOL
const betAmount = new anchor.BN(0.1 * anchor.web3.LAMPORTS_PER_SOL);
await program.methods
  .placeDiceBetSol(50, { under: {} }, betAmount)
  .accounts({
    vault: vaultPda,
    bet: betPda,
    player: player.publicKey,
    playerAta: player.publicKey,
    vaultAta: vaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([player])
  .rpc();
```

#### Resolve Dice Bet

```typescript
// Resolve with VRF result (from Switchboard or other VRF)
const vrfResult = Buffer.from(/* VRF random bytes */);

await program.methods
  .resolveDiceBet(Array.from(vrfResult))
  .accounts({
    vault: vaultPda,
    bet: betPda,
    playerAta: player.publicKey,
    vaultAta: vaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .rpc();
```

#### Place Plinko Bet

```typescript
// Place Plinko bet with 8 rows, bet amount 0.1 SOL
const betAmount = new anchor.BN(0.1 * anchor.web3.LAMPORTS_PER_SOL);
await program.methods
  .placePlinkoBetSol(8, betAmount)
  .accounts({
    vault: vaultPda,
    bet: betPda,
    player: player.publicKey,
    playerAta: player.publicKey,
    vaultAta: vaultPda,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([player])
  .rpc();
```

#### Admin Functions

```typescript
// Update house edge (admin only)
await program.methods
  .updateHouseEdge(300) // 3%
  .accounts({
    vault: vaultPda,
    admin: admin.publicKey,
  })
  .rpc();

// Update bet limits (admin only)
await program.methods
  .updateBetLimits(
    new anchor.BN(50000),  // new min_bet
    new anchor.BN(20 * anchor.web3.LAMPORTS_PER_SOL) // new max_bet
  )
  .accounts({
    vault: vaultPda,
    admin: admin.publicKey,
  })
  .rpc();
```

## 🏗️ Architecture

### Program Structure

```
programs/solana-casino-smart-contract/
├── src/
│   ├── lib.rs          # Main program entry, instructions, account contexts
│   ├── dice.rs         # Dice game logic and payout calculations
│   ├── plinko.rs       # Plinko game logic and resolution
│   ├── vrf.rs          # VRF integration helpers
│   ├── state.rs        # Account structures (CasinoVault, DiceBet, PlinkoBet)
│   └── error.rs        # Custom error definitions
```

### Account Structure

#### CasinoVault
- `admin`: Pubkey of the admin (can update house edge and limits)
- `house_edge_bps`: House edge in basis points (100 = 1%, max 500 = 5%)
- `min_bet`: Minimum bet amount in lamports
- `max_bet`: Maximum bet amount in lamports
- `sol_balance`: Current SOL balance in vault
- `bump`: PDA bump seed

#### DiceBet
- `player`: Player's public key
- `amount`: Bet amount
- `target`: Target number (1-100)
- `direction`: Under or Over
- `vrf_request`: Optional VRF request account
- `random_value`: Resolved random value (1-100)
- `payout`: Calculated payout (None if lost)
- `resolved`: Whether bet is resolved
- `mint`: Token mint (None for SOL)
- `bump`: PDA bump seed
- `created_at`: Timestamp

#### PlinkoBet
- `player`: Player's public key
- `amount`: Bet amount
- `rows`: Number of rows (8-16)
- `vrf_request`: Optional VRF request account
- `path`: Resolved path (left=0, right=1)
- `multiplier`: Resolved multiplier in basis points
- `payout`: Calculated payout
- `resolved`: Whether bet is resolved
- `mint`: Token mint (None for SOL)
- `bump`: PDA bump seed
- `created_at`: Timestamp

### Instruction Flow

1. **Initialize**: Admin initializes casino vault with house edge and bet limits.
2. **Place Bet**: Player places bet (SOL or token), funds are transferred to vault, bet account is created.
3. **Request VRF**: (External) Request randomness from VRF oracle (Switchboard, etc.).
4. **Resolve Bet**: VRF callback resolves bet, calculates payout, transfers winnings if player won.

### Security Features

- **PDA-based accounts**: All accounts use PDAs (Program Derived Addresses) - no hardcoded keys
- **Access control**: Admin-only functions for house edge and bet limits
- **Overflow protection**: All arithmetic uses checked operations
- **Reentrancy protection**: State checks prevent double resolution
- **Bet validation**: Min/max bet limits enforced
- **Input validation**: All inputs validated (target ranges, row counts, etc.)

## 🎲 Game Mechanics

### Dice Game

- **Target Range**: 1-100
- **Bet Direction**: Under or Over the target
- **Payout Formula**: `(100 / win_probability) * (1 - house_edge)`
  - Example: Roll under 50 has 49% win chance → payout ≈ 2.0x (minus house edge)
- **Win Condition**: 
  - Under: Roll < target
  - Over: Roll > target

### Plinko Game

- **Rows**: 8, 12, or 16 rows
- **Path**: Determined by VRF (left=0, right=1 for each row)
- **Multiplier**: Based on final position (bin)
  - 8 rows: 0.5x to 10x
  - 12 rows: 0.2x to 20x
  - 16 rows: 0.2x to 100x
- **Payout**: `bet_amount * multiplier * (1 - house_edge)`


## 📧 Support

- telegram: https://t.me/CasinoCutup
- twitter:  https://x.com/CasinoCutup
