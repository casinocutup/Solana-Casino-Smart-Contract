// Solana Casino Smart Contract
// Provably fair on-chain casino games with Dice and Plinko
// Unaudited code — use at own risk.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use anchor_lang::system_program;

mod dice;
mod error;
mod plinko;
mod state;
mod vrf;

use dice::*;
use error::CasinoError;
use plinko::*;
use state::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod solana_casino_smart_contract {
    use super::*;

    /// Initialize the casino vault with house configuration
    /// 
    /// # Arguments
    /// * `house_edge_bps` - House edge in basis points (100 = 1%, max 500 = 5%)
    /// * `min_bet` - Minimum bet amount in lamports
    /// * `max_bet` - Maximum bet amount in lamports
    pub fn initialize_casino(
        ctx: Context<InitializeCasino>,
        house_edge_bps: u16,
        min_bet: u64,
        max_bet: u64,
    ) -> Result<()> {
        // Validate house edge (1-5%)
        if house_edge_bps < 100 || house_edge_bps > 500 {
            return Err(error!(CasinoError::InvalidHouseEdge));
        }
        
        // Validate bet limits
        if min_bet == 0 || min_bet > max_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        
        let vault = &mut ctx.accounts.vault;
        vault.admin = ctx.accounts.admin.key();
        vault.house_edge_bps = house_edge_bps;
        vault.min_bet = min_bet;
        vault.max_bet = max_bet;
        vault.sol_balance = 0;
        vault.bump = ctx.bumps.vault;
        
        msg!("Casino initialized with house edge: {} bps", house_edge_bps);
        Ok(())
    }

    /// Update house edge (admin only)
    pub fn update_house_edge(
        ctx: Context<UpdateHouseEdge>,
        new_house_edge_bps: u16,
    ) -> Result<()> {
        // Validate house edge
        if new_house_edge_bps < 100 || new_house_edge_bps > 500 {
            return Err(error!(CasinoError::InvalidHouseEdge));
        }
        
        // Check admin
        if ctx.accounts.vault.admin != ctx.accounts.admin.key() {
            return Err(error!(CasinoError::Unauthorized));
        }
        
        ctx.accounts.vault.house_edge_bps = new_house_edge_bps;
        msg!("House edge updated to: {} bps", new_house_edge_bps);
        Ok(())
    }

    /// Update bet limits (admin only)
    pub fn update_bet_limits(
        ctx: Context<UpdateBetLimits>,
        min_bet: u64,
        max_bet: u64,
    ) -> Result<()> {
        // Check admin
        if ctx.accounts.vault.admin != ctx.accounts.admin.key() {
            return Err(error!(CasinoError::Unauthorized));
        }
        
        // Validate bet limits
        if min_bet == 0 || min_bet > max_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        
        ctx.accounts.vault.min_bet = min_bet;
        ctx.accounts.vault.max_bet = max_bet;
        msg!("Bet limits updated: min={}, max={}", min_bet, max_bet);
        Ok(())
    }

    /// Place a dice bet (SOL)
    /// 
    /// # Arguments
    /// * `target` - Target number (1-100)
    /// * `direction` - Bet direction (Under or Over)
    /// * `amount` - Bet amount in lamports
    pub fn place_dice_bet_sol(
        ctx: Context<PlaceDiceBetSol>,
        target: u8,
        direction: BetDirection,
        amount: u64,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let player = &ctx.accounts.player;
        let bet = &mut ctx.accounts.bet;
        
        // Validate target
        if target < 1 || target > 100 {
            return Err(error!(CasinoError::InvalidDiceTarget));
        }
        
        // Validate bet amount
        if amount < vault.min_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        if amount > vault.max_bet {
            return Err(error!(CasinoError::BetAboveMaximum));
        }
        
        // Check player has enough SOL
        let player_lamports = ctx.accounts.player.to_account_info().lamports();
        let rent_exempt_min = Rent::get()?.minimum_balance(0);
        if player_lamports < amount + rent_exempt_min {
            return Err(error!(CasinoError::InsufficientFunds));
        }
        
        let bet_amount = amount;
        
        // Validate bet amount
        if bet_amount < vault.min_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        if bet_amount > vault.max_bet {
            return Err(error!(CasinoError::BetAboveMaximum));
        }
        
        // Initialize bet account
        bet.player = player.key();
        bet.amount = bet_amount;
        bet.target = target;
        bet.direction = direction;
        bet.vrf_request = None;
        bet.random_value = None;
        bet.payout = None;
        bet.resolved = false;
        bet.mint = None;
        bet.bump = ctx.bumps.bet;
        bet.created_at = Clock::get()?.unix_timestamp;
        
        // Transfer SOL to vault using SystemProgram
        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.player.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program = ctx.accounts.system_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        system_program::transfer(cpi_ctx, amount)?;
        
        // Update vault balance
        ctx.accounts.vault.sol_balance = ctx.accounts.vault.to_account_info().lamports();
        
        msg!("Dice bet placed: {} lamports, target={}, direction={:?}", 
             amount, target, direction);
        
        Ok(())
    }

    /// Place a dice bet (SPL token)
    pub fn place_dice_bet_token(
        ctx: Context<PlaceDiceBetToken>,
        target: u8,
        direction: BetDirection,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let player = &ctx.accounts.player;
        let bet = &mut ctx.accounts.bet;
        
        // Validate target
        if target < 1 || target > 100 {
            return Err(error!(CasinoError::InvalidDiceTarget));
        }
        
        // Get bet amount
        let bet_amount = ctx.accounts.player_token_account.amount;
        
        // Validate bet amount
        if bet_amount < vault.min_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        if bet_amount > vault.max_bet {
            return Err(error!(CasinoError::BetAboveMaximum));
        }
        
        // Transfer tokens to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.player_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: player.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, bet_amount)?;
        
        // Initialize bet account
        bet.player = player.key();
        bet.amount = bet_amount;
        bet.target = target;
        bet.direction = direction;
        bet.vrf_request = None;
        bet.random_value = None;
        bet.payout = None;
        bet.resolved = false;
        bet.mint = Some(ctx.accounts.mint.key());
        bet.bump = ctx.bumps.bet;
        bet.created_at = Clock::get()?.unix_timestamp;
        
        msg!("Dice bet placed (token): {} tokens, target={}, direction={:?}", 
             bet_amount, target, direction);
        
        Ok(())
    }

    /// Resolve a dice bet with VRF result
    pub fn resolve_dice_bet(
        ctx: Context<ResolveDiceBet>,
        vrf_result: Vec<u8>,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let bet = &mut ctx.accounts.bet;
        
        // Check if already resolved
        if bet.resolved {
            return Err(error!(CasinoError::BetAlreadyResolved));
        }
        
        // Resolve bet
        let payout = resolve_dice_bet(bet, &vrf_result, vault.house_edge_bps)?;
        
        // Transfer winnings if player won
        if payout > 0 {
            if bet.mint.is_none() {
                // SOL payout
                let vault_lamports = ctx.accounts.vault.to_account_info().lamports();
                if vault_lamports < payout {
                    return Err(error!(CasinoError::InsufficientFunds));
                }
                
                // Transfer SOL from vault to player
                let cpi_accounts = system_program::Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.player_ata.to_account_info(),
                };
                let cpi_program = ctx.accounts.system_program.to_account_info();
                let seeds = &[
                    b"vault",
                    &[ctx.accounts.vault.bump],
                ];
                let signer = &[&seeds[..]];
                let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
                system_program::transfer(cpi_ctx, payout)?;
                
                ctx.accounts.vault.sol_balance = ctx.accounts.vault.to_account_info().lamports();
            } else {
                // Token payout
                let mint = bet.mint.unwrap();
                if mint != ctx.accounts.mint.key() {
                    return Err(error!(CasinoError::InvalidTokenMint));
                }
                
                let seeds = &[
                    b"vault",
                    &[ctx.accounts.vault.bump],
                ];
                let signer = &[&seeds[..]];
                
                let cpi_accounts = Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.player_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
                token::transfer(cpi_ctx, payout)?;
            }
            
            msg!("Dice bet resolved: Player won {} (roll: {:?})", 
                 payout, bet.random_value);
        } else {
            msg!("Dice bet resolved: Player lost (roll: {:?})", 
                 bet.random_value);
        }
        
        Ok(())
    }

    /// Place a Plinko bet (SOL)
    /// 
    /// # Arguments
    /// * `rows` - Number of rows (8-16)
    /// * `amount` - Bet amount in lamports
    pub fn place_plinko_bet_sol(
        ctx: Context<PlacePlinkoBetSol>,
        rows: u8,
        amount: u64,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let player = &ctx.accounts.player;
        let bet = &mut ctx.accounts.bet;
        
        // Validate rows
        if rows < 8 || rows > 16 {
            return Err(error!(CasinoError::InvalidPlinkoRows));
        }
        
        // Validate bet amount
        if amount < vault.min_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        if amount > vault.max_bet {
            return Err(error!(CasinoError::BetAboveMaximum));
        }
        
        // Check player has enough SOL
        let player_lamports = ctx.accounts.player.to_account_info().lamports();
        let rent_exempt_min = Rent::get()?.minimum_balance(0);
        if player_lamports < amount + rent_exempt_min {
            return Err(error!(CasinoError::InsufficientFunds));
        }
        
        // Initialize bet account
        bet.player = player.key();
        bet.amount = amount;
        bet.rows = rows;
        bet.vrf_request = None;
        bet.path = None;
        bet.multiplier = None;
        bet.payout = None;
        bet.resolved = false;
        bet.mint = None;
        bet.bump = ctx.bumps.bet;
        bet.created_at = Clock::get()?.unix_timestamp;
        
        // Transfer SOL to vault using SystemProgram
        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.player.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        };
        let cpi_program = ctx.accounts.system_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        system_program::transfer(cpi_ctx, amount)?;
        
        // Update vault balance
        ctx.accounts.vault.sol_balance = ctx.accounts.vault.to_account_info().lamports();
        
        msg!("Plinko bet placed: {} lamports, rows={}", amount, rows);
        
        Ok(())
    }

    /// Place a Plinko bet (SPL token)
    pub fn place_plinko_bet_token(
        ctx: Context<PlacePlinkoBetToken>,
        rows: u8,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let player = &ctx.accounts.player;
        let bet = &mut ctx.accounts.bet;
        
        // Validate rows
        if rows < 8 || rows > 16 {
            return Err(error!(CasinoError::InvalidPlinkoRows));
        }
        
        // Get bet amount
        let bet_amount = ctx.accounts.player_token_account.amount;
        
        // Validate bet amount
        if bet_amount < vault.min_bet {
            return Err(error!(CasinoError::BetBelowMinimum));
        }
        if bet_amount > vault.max_bet {
            return Err(error!(CasinoError::BetAboveMaximum));
        }
        
        // Transfer tokens to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.player_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: player.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, bet_amount)?;
        
        // Initialize bet account
        bet.player = player.key();
        bet.amount = bet_amount;
        bet.rows = rows;
        bet.vrf_request = None;
        bet.path = None;
        bet.multiplier = None;
        bet.payout = None;
        bet.resolved = false;
        bet.mint = Some(ctx.accounts.mint.key());
        bet.bump = ctx.bumps.bet;
        bet.created_at = Clock::get()?.unix_timestamp;
        
        msg!("Plinko bet placed (token): {} tokens, rows={}", bet_amount, rows);
        
        Ok(())
    }

    /// Resolve a Plinko bet with VRF result
    pub fn resolve_plinko_bet(
        ctx: Context<ResolvePlinkoBet>,
        vrf_result: Vec<u8>,
    ) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let bet = &mut ctx.accounts.bet;
        
        // Check if already resolved
        if bet.resolved {
            return Err(error!(CasinoError::BetAlreadyResolved));
        }
        
        // Resolve bet
        let payout = resolve_plinko_bet(bet, &vrf_result, vault.house_edge_bps)?;
        
        // Transfer winnings if player won
        if payout > 0 {
            if bet.mint.is_none() {
                // SOL payout
                let vault_lamports = ctx.accounts.vault.to_account_info().lamports();
                if vault_lamports < payout {
                    return Err(error!(CasinoError::InsufficientFunds));
                }
                
                // Transfer SOL from vault to player
                let cpi_accounts = system_program::Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.player_ata.to_account_info(),
                };
                let cpi_program = ctx.accounts.system_program.to_account_info();
                let seeds = &[
                    b"vault",
                    &[ctx.accounts.vault.bump],
                ];
                let signer = &[&seeds[..]];
                let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
                system_program::transfer(cpi_ctx, payout)?;
                
                ctx.accounts.vault.sol_balance = ctx.accounts.vault.to_account_info().lamports();
            } else {
                // Token payout
                let mint = bet.mint.unwrap();
                if mint != ctx.accounts.mint.key() {
                    return Err(error!(CasinoError::InvalidTokenMint));
                }
                
                let seeds = &[
                    b"vault",
                    &[ctx.accounts.vault.bump],
                ];
                let signer = &[&seeds[..]];
                
                let cpi_accounts = Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.player_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
                token::transfer(cpi_ctx, payout)?;
            }
            
            msg!("Plinko bet resolved: Player won {} (multiplier: {:?} bps)", 
                 payout, bet.multiplier);
        } else {
            msg!("Plinko bet resolved: Player lost");
        }
        
        Ok(())
    }
}

// Account contexts

#[derive(Accounts)]
pub struct InitializeCasino<'info> {
    #[account(
        init,
        payer = admin,
        space = CasinoVault::LEN,
        seeds = [b"vault"],
        bump
    )]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(mut)]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateHouseEdge<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateBetLimits<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct PlaceDiceBetSol<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(
        init,
        payer = player,
        space = DiceBet::LEN,
        seeds = [b"dice_bet", player.key().as_ref(), &Clock::get()?.unix_timestamp.to_le_bytes()],
        bump
    )]
    pub bet: Account<'info, DiceBet>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    /// CHECK: Player's SOL account
    #[account(mut)]
    pub player_ata: AccountInfo<'info>,
    
    /// CHECK: Vault's SOL account
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceDiceBetToken<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(
        init,
        payer = player,
        space = DiceBet::LEN,
        seeds = [b"dice_bet", player.key().as_ref(), &Clock::get()?.unix_timestamp.to_le_bytes()],
        bump
    )]
    pub bet: Account<'info, DiceBet>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    #[account(mut)]
    pub player_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    
    pub mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveDiceBet<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(mut)]
    pub bet: Account<'info, DiceBet>,
    
    /// CHECK: Player's account (SOL or token)
    #[account(mut)]
    pub player_ata: AccountInfo<'info>,
    
    /// CHECK: Vault's account (SOL or token)
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,
    
    /// CHECK: Token mint (if token bet)
    pub mint: Option<Account<'info, Mint>>,
    
    /// CHECK: Token program (if token bet)
    pub token_program: Option<Program<'info, Token>>,
    
    /// CHECK: Player's token account (if token bet)
    #[account(mut)]
    pub player_token_account: Option<Account<'info, TokenAccount>>,
    
    /// CHECK: Vault's token account (if token bet)
    #[account(mut)]
    pub vault_token_account: Option<Account<'info, TokenAccount>>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlacePlinkoBetSol<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(
        init,
        payer = player,
        space = PlinkoBet::LEN,
        seeds = [b"plinko_bet", player.key().as_ref(), &Clock::get()?.unix_timestamp.to_le_bytes()],
        bump
    )]
    pub bet: Account<'info, PlinkoBet>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    /// CHECK: Player's SOL account
    #[account(mut)]
    pub player_ata: AccountInfo<'info>,
    
    /// CHECK: Vault's SOL account
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlacePlinkoBetToken<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(
        init,
        payer = player,
        space = PlinkoBet::LEN,
        seeds = [b"plinko_bet", player.key().as_ref(), &Clock::get()?.unix_timestamp.to_le_bytes()],
        bump
    )]
    pub bet: Account<'info, PlinkoBet>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    #[account(mut)]
    pub player_token_account: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub vault_token_account: Account<'info, TokenAccount>,
    
    pub mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolvePlinkoBet<'info> {
    #[account(mut)]
    pub vault: Account<'info, CasinoVault>,
    
    #[account(mut)]
    pub bet: Account<'info, PlinkoBet>,
    
    /// CHECK: Player's account (SOL or token)
    #[account(mut)]
    pub player_ata: AccountInfo<'info>,
    
    /// CHECK: Vault's account (SOL or token)
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,
    
    /// CHECK: Token mint (if token bet)
    pub mint: Option<Account<'info, Mint>>,
    
    /// CHECK: Token program (if token bet)
    pub token_program: Option<Program<'info, Token>>,
    
    /// CHECK: Player's token account (if token bet)
    #[account(mut)]
    pub player_token_account: Option<Account<'info, TokenAccount>>,
    
    /// CHECK: Vault's token account (if token bet)
    #[account(mut)]
    pub vault_token_account: Option<Account<'info, TokenAccount>>,
    
    pub system_program: Program<'info, System>,
}
