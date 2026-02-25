use anchor_lang::prelude::*;

#[error_code]
pub enum CasinoError {
    #[msg("Insufficient funds in vault")]
    InsufficientFunds,
    
    #[msg("Bet amount below minimum")]
    BetBelowMinimum,
    
    #[msg("Bet amount above maximum")]
    BetAboveMaximum,
    
    #[msg("Invalid dice roll target (must be 1-100)")]
    InvalidDiceTarget,
    
    #[msg("Invalid bet direction (must be under or over)")]
    InvalidBetDirection,
    
    #[msg("Invalid Plinko rows (must be 8-16)")]
    InvalidPlinkoRows,
    
    #[msg("VRF request failed")]
    VrfRequestFailed,
    
    #[msg("VRF callback not authorized")]
    VrfCallbackUnauthorized,
    
    #[msg("Bet not found or already resolved")]
    BetNotFound,
    
    #[msg("House edge out of valid range (1-5%)")]
    InvalidHouseEdge,
    
    #[msg("Unauthorized: Admin access required")]
    Unauthorized,
    
    #[msg("Math overflow")]
    MathOverflow,
    
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    
    #[msg("Bet already resolved")]
    BetAlreadyResolved,
    
    #[msg("VRF randomness not ready")]
    VrfNotReady,
}
