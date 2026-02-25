// Plinko game logic
// Unaudited code — use at own risk.

use anchor_lang::prelude::*;
use crate::error::CasinoError;
use crate::state::PlinkoBet;
use crate::vrf;

/// Resolve a Plinko bet with VRF result
pub fn resolve_plinko_bet(
    bet: &mut PlinkoBet,
    vrf_bytes: &[u8],
    house_edge_bps: u16,
) -> Result<u64> {
    // Check if already resolved
    if bet.resolved {
        return Err(error!(CasinoError::BetAlreadyResolved));
    }
    
    // Validate rows
    if bet.rows < 8 || bet.rows > 16 {
        return Err(error!(CasinoError::InvalidPlinkoRows));
    }
    
    // Generate path from VRF
    let path = vrf::vrf_to_plinko_path(vrf_bytes, bet.rows)?;
    bet.path = Some(path.clone());
    
    // Calculate final position (number of "right" moves)
    let position = path.iter().sum::<u8>();
    
    // Get multiplier for this position
    let multiplier_bps = vrf::calculate_plinko_multiplier(position, bet.rows)?;
    bet.multiplier = Some(multiplier_bps);
    
    // Apply house edge to multiplier
    let house_edge_multiplier = 10000u64
        .checked_sub(house_edge_bps as u64)
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    let final_multiplier_bps = multiplier_bps
        .checked_mul(house_edge_multiplier)
        .and_then(|x| x.checked_div(10000))
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    // Calculate payout
    let payout = bet.amount
        .checked_mul(final_multiplier_bps)
        .and_then(|x| x.checked_div(10000))
        .ok_or(error!(CasinoError::MathOverflow))?;
    
    bet.payout = Some(payout);
    bet.resolved = true;
    
    Ok(payout)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plinko_resolution() {
        let mut bet = PlinkoBet {
            player: anchor_lang::solana_program::pubkey!("11111111111111111111111111111111"),
            amount: 1000000, // 0.001 SOL
            rows: 8,
            vrf_request: None,
            path: None,
            multiplier: None,
            payout: None,
            resolved: false,
            mint: None,
            bump: 0,
            created_at: 0,
        };
        
        // Mock VRF bytes
        let vrf_bytes = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0];
        let house_edge = 200; // 2%
        
        let payout = resolve_plinko_bet(&mut bet, &vrf_bytes, house_edge).unwrap();
        assert!(bet.resolved);
        assert!(bet.path.is_some());
        assert!(bet.multiplier.is_some());
        assert!(bet.payout.is_some());
    }
}
