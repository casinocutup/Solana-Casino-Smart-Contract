// Dice game logic
// Unaudited code — use at own risk.

use anchor_lang::prelude::*;
use crate::error::CasinoError;
use crate::state::{DiceBet, BetDirection};
use crate::vrf;

/// Calculate dice payout based on target, direction, and roll result
/// Payout formula: (100 / win_probability) * (1 - house_edge)
/// Example: Roll under 50 has 49% win chance (1-49), payout = (100/49) * 0.98 = ~2.0x
pub fn calculate_dice_payout(
    target: u8,
    direction: BetDirection,
    roll: u8,
    house_edge_bps: u16,
) -> Result<Option<u64>> {
    // Validate inputs
    if target < 1 || target > 100 {
        return Err(error!(CasinoError::InvalidDiceTarget));
    }
    
    if roll < 1 || roll > 100 {
        return Err(error!(CasinoError::InvalidDiceTarget));
    }
    
    // Determine if player won
    let won = match direction {
        BetDirection::Under => roll < target,
        BetDirection::Over => roll > target,
    };
    
    if !won {
        return Ok(None); // Player lost
    }
    
    // Calculate win probability
    let win_probability = match direction {
        BetDirection::Under => {
            // Roll under target: can roll 1 to (target-1)
            if target <= 1 {
                return Ok(None); // Impossible to win
            }
            (target - 1) as u64
        }
        BetDirection::Over => {
            // Roll over target: can roll (target+1) to 100
            if target >= 100 {
                return Ok(None); // Impossible to win
            }
            (100 - target) as u64
        }
    };
    
    // Calculate payout multiplier (before house edge)
    // Payout = (100 / win_probability) * (1 - house_edge)
    // We work in basis points: 10000 = 1.0x, 20000 = 2.0x
    let payout_multiplier_bps = (10000u64)
        .checked_mul(100)
        .and_then(|x| x.checked_div(win_probability))
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    // Apply house edge
    let house_edge_multiplier = 10000u64
        .checked_sub(house_edge_bps as u64)
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    let final_multiplier_bps = payout_multiplier_bps
        .checked_mul(house_edge_multiplier)
        .and_then(|x| x.checked_div(10000))
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    Ok(Some(final_multiplier_bps))
}

/// Resolve a dice bet with VRF result
pub fn resolve_dice_bet(
    bet: &mut DiceBet,
    vrf_bytes: &[u8],
    house_edge_bps: u16,
) -> Result<u64> {
    // Check if already resolved
    if bet.resolved {
        return Err(error!(CasinoError::BetAlreadyResolved));
    }
    
    // Convert VRF to dice roll (1-100)
    let roll = vrf::vrf_to_dice_roll(vrf_bytes)?;
    bet.random_value = Some(roll as u64);
    
    // Calculate payout
    let multiplier_bps = calculate_dice_payout(
        bet.target,
        bet.direction,
        roll,
        house_edge_bps,
    )?;
    
    if let Some(mult_bps) = multiplier_bps {
        // Player won - calculate payout
        let payout = bet.amount
            .checked_mul(mult_bps)
            .and_then(|x| x.checked_div(10000))
            .ok_or(error!(CasinoError::MathOverflow))?;
        
        bet.payout = Some(payout);
        bet.resolved = true;
        Ok(payout)
    } else {
        // Player lost
        bet.payout = None;
        bet.resolved = true;
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dice_payout_under_50() {
        // Roll under 50: 49% win chance
        // Expected payout: (100/49) * 0.98 ≈ 2.0x
        let house_edge = 200; // 2%
        let multiplier = calculate_dice_payout(50, BetDirection::Under, 25, house_edge).unwrap();
        assert!(multiplier.is_some());
        // Should be around 20000 basis points (2.0x)
        assert!(multiplier.unwrap() > 19000 && multiplier.unwrap() < 21000);
    }
    
    #[test]
    fn test_dice_payout_over_50() {
        // Roll over 50: 49% win chance
        let house_edge = 200;
        let multiplier = calculate_dice_payout(50, BetDirection::Over, 75, house_edge).unwrap();
        assert!(multiplier.is_some());
    }
    
    #[test]
    fn test_dice_loss() {
        // Roll 50 when betting under 50 should lose
        let house_edge = 200;
        let multiplier = calculate_dice_payout(50, BetDirection::Under, 50, house_edge).unwrap();
        assert!(multiplier.is_none());
    }
}
