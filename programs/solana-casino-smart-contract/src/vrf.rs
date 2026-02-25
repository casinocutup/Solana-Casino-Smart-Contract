// VRF integration helpers
// This module provides utilities for working with Switchboard VRF
// Unaudited code — use at own risk.

use anchor_lang::prelude::*;

/// Calculate a random value from VRF result
/// For Switchboard VRF, the randomness is provided as bytes
/// We convert it to a u64 value in the range [0, max_value)
pub fn vrf_to_u64(vrf_bytes: &[u8], max_value: u64) -> Result<u64> {
    if vrf_bytes.len() < 8 {
        return Err(anchor_lang::error!(crate::error::CasinoError::VrfNotReady));
    }
    
    // Take first 8 bytes and convert to u64
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&vrf_bytes[..8]);
    let value = u64::from_le_bytes(bytes);
    
    // Modulo to get value in range [0, max_value)
    Ok(value % max_value)
}

/// Generate a random number in range [1, 100] for dice
pub fn vrf_to_dice_roll(vrf_bytes: &[u8]) -> Result<u8> {
    let value = vrf_to_u64(vrf_bytes, 100)?;
    // Convert to 1-100 range
    Ok((value as u8) + 1)
}

/// Generate random bits for Plinko path (0 = left, 1 = right)
pub fn vrf_to_plinko_path(vrf_bytes: &[u8], rows: u8) -> Result<Vec<u8>> {
    if rows < 8 || rows > 16 {
        return Err(anchor_lang::error!(crate::error::CasinoError::InvalidPlinkoRows));
    }
    
    let mut path = Vec::with_capacity(rows as usize);
    
    // Use VRF bytes to determine path
    for i in 0..rows {
        let byte_index = (i / 8) as usize;
        if byte_index < vrf_bytes.len() {
            let bit_index = (i % 8) as u8;
            let bit = (vrf_bytes[byte_index] >> bit_index) & 1;
            path.push(bit);
        } else {
            // Fallback: use modulo of index
            path.push((i % 2) as u8);
        }
    }
    
    Ok(path)
}

/// Calculate Plinko multiplier based on final position
/// Position is the number of "right" moves (0 to rows)
/// Multipliers are configured based on position
pub fn calculate_plinko_multiplier(position: u8, rows: u8) -> Result<u64> {
    if rows < 8 || rows > 16 {
        return Err(anchor_lang::error!(crate::error::CasinoError::InvalidPlinkoRows));
    }
    
    // Multiplier table based on position
    // This is a simplified version - in production, you'd want more sophisticated multipliers
    let multiplier_bps = match rows {
        8 => match position {
            0 | 8 => 1000,  // 10x
            1 | 7 => 500,   // 5x
            2 | 6 => 200,   // 2x
            3 | 5 => 150,   // 1.5x
            4 => 100,       // 1x
            _ => 50,        // 0.5x
        },
        12 => match position {
            0 | 12 => 2000, // 20x
            1 | 11 => 1000, // 10x
            2 | 10 => 500,  // 5x
            3 | 9 => 200,   // 2x
            4 | 8 => 150,   // 1.5x
            5 | 7 => 100,   // 1x
            6 => 50,        // 0.5x
            _ => 20,        // 0.2x
        },
        16 => match position {
            0 | 16 => 10000, // 100x
            1 | 15 => 5000,  // 50x
            2 | 14 => 2000,  // 20x
            3 | 13 => 1000,  // 10x
            4 | 12 => 500,   // 5x
            5 | 11 => 200,   // 2x
            6 | 10 => 150,   // 1.5x
            7 | 9 => 100,    // 1x
            8 => 50,         // 0.5x
            _ => 20,         // 0.2x
        },
        _ => {
            // Default multiplier calculation for other row counts
            let center = rows / 2;
            let distance = if position > center {
                (position - center) as i16
            } else {
                (center - position) as i16
            };
            
            // Closer to center = lower multiplier
            // Further from center = higher multiplier
            let base_multiplier = 100; // 1x base
            let bonus = (distance as u64) * 50; // 0.5x per step from center
            base_multiplier + bonus
        }
    };
    
    Ok(multiplier_bps)
}
