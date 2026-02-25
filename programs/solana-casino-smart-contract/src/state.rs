use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

// Casino vault/house configuration
#[account]
pub struct CasinoVault {
    pub admin: Pubkey,              // Admin who can update house edge
    pub house_edge_bps: u16,        // House edge in basis points (100 = 1%, max 500 = 5%)
    pub min_bet: u64,               // Minimum bet amount
    pub max_bet: u64,               // Maximum bet amount
    pub sol_balance: u64,           // SOL balance in vault
    pub bump: u8,                   // PDA bump seed
}

impl CasinoVault {
    pub const LEN: usize = 8 + // discriminator
        32 +                    // admin
        2 +                     // house_edge_bps
        8 +                     // min_bet
        8 +                     // max_bet
        8 +                     // sol_balance
        1;                      // bump
}

// Dice bet account
#[account]
pub struct DiceBet {
    pub player: Pubkey,           // Player who placed the bet
    pub amount: u64,              // Bet amount
    pub target: u8,               // Target number (1-100)
    pub direction: BetDirection,  // Under or over
    pub vrf_request: Option<Pubkey>, // VRF request account (if using Switchboard)
    pub random_value: Option<u64>,   // Resolved random value
    pub payout: Option<u64>,         // Calculated payout (None if lost)
    pub resolved: bool,              // Whether bet is resolved
    pub mint: Option<Pubkey>,        // Token mint (None for SOL)
    pub bump: u8,                    // PDA bump seed
    pub created_at: i64,             // Timestamp
}

impl DiceBet {
    pub const LEN: usize = 8 +  // discriminator
        32 +                    // player
        8 +                     // amount
        1 +                     // target
        1 +                     // direction
        1 + 32 +                // vrf_request (Option)
        1 + 8 +                 // random_value (Option)
        1 + 8 +                 // payout (Option)
        1 +                     // resolved
        1 + 32 +                // mint (Option)
        1 +                     // bump
        8;                      // created_at
}

// Plinko bet account
#[account]
pub struct PlinkoBet {
    pub player: Pubkey,           // Player who placed the bet
    pub amount: u64,              // Bet amount
    pub rows: u8,                 // Number of rows (8-16)
    pub vrf_request: Option<Pubkey>, // VRF request account
    pub path: Option<Vec<u8>>,      // Resolved path (left=0, right=1)
    pub multiplier: Option<u64>,    // Resolved multiplier in basis points
    pub payout: Option<u64>,        // Calculated payout
    pub resolved: bool,             // Whether bet is resolved
    pub mint: Option<Pubkey>,       // Token mint (None for SOL)
    pub bump: u8,                   // PDA bump seed
    pub created_at: i64,            // Timestamp
}

impl PlinkoBet {
    pub const LEN: usize = 8 +  // discriminator
        32 +                    // player
        8 +                     // amount
        1 +                     // rows
        1 + 32 +                // vrf_request (Option)
        4 + 16 +                // path (Option<Vec<u8>>, max 16 bytes)
        1 + 8 +                 // multiplier (Option)
        1 + 8 +                 // payout (Option)
        1 +                     // resolved
        1 + 32 +                // mint (Option)
        1 +                     // bump
        8;                      // created_at
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub enum BetDirection {
    Under,
    Over,
}
