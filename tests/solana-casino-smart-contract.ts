import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaCasinoSmartContract } from "../target/types/solana_casino_smart_contract";
import { expect } from "chai";
import { 
  PublicKey, 
  Keypair, 
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
} from "@solana/web3.js";
import { 
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createInitializeMintInstruction,
  getMinimumBalanceForRentExemptMint,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";

describe("solana-casino-smart-contract", () => {
  // Configure the client
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolanaCasinoSmartContract as Program<SolanaCasinoSmartContract>;
  
  const admin = provider.wallet;
  const player = Keypair.generate();
  
  let vaultPda: PublicKey;
  let vaultBump: number;
  
  const HOUSE_EDGE_BPS = 200; // 2%
  const MIN_BET = new anchor.BN(100000); // 0.0001 SOL
  const MAX_BET = new anchor.BN(10 * LAMPORTS_PER_SOL); // 10 SOL

  before(async () => {
    // Airdrop SOL to player
    const signature = await provider.connection.requestAirdrop(
      player.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature);
  });

  describe("Initialize Casino", () => {
    it("Initializes the casino vault", async () => {
      [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault")],
        program.programId
      );

      const tx = await program.methods
        .initializeCasino(HOUSE_EDGE_BPS, MIN_BET, MAX_BET)
        .accounts({
          vault: vaultPda,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      console.log("Initialize transaction:", tx);

      const vault = await program.account.casinoVault.fetch(vaultPda);
      expect(vault.admin.toString()).to.equal(admin.publicKey.toString());
      expect(vault.houseEdgeBps).to.equal(HOUSE_EDGE_BPS);
      expect(vault.minBet.toString()).to.equal(MIN_BET.toString());
      expect(vault.maxBet.toString()).to.equal(MAX_BET.toString());
    });

    it("Fails with invalid house edge", async () => {
      const invalidVault = Keypair.generate();
      
      try {
        await program.methods
          .initializeCasino(600, MIN_BET, MAX_BET) // 6% > 5% max
          .accounts({
            vault: invalidVault.publicKey,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        
        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err.toString()).to.include("InvalidHouseEdge");
      }
    });
  });

  describe("Dice Game (SOL)", () => {
    let betPda: PublicKey;
    const TARGET = 50;
    const BET_AMOUNT = new anchor.BN(0.1 * LAMPORTS_PER_SOL);

    it("Places a dice bet (under)", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      [betPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("dice_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      // Transfer SOL to player account for bet
      const transferTx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: player.publicKey,
          lamports: BET_AMOUNT.toNumber(),
        })
      );
      await provider.sendAndConfirm(transferTx);

      const tx = await program.methods
        .placeDiceBetSol(TARGET, { under: {} }, BET_AMOUNT)
        .accounts({
          vault: vaultPda,
          bet: betPda,
          player: player.publicKey,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([player])
        .rpc();

      console.log("Dice bet transaction:", tx);

      const bet = await program.account.diceBet.fetch(betPda);
      expect(bet.player.toString()).to.equal(player.publicKey.toString());
      expect(bet.amount.toString()).to.equal(BET_AMOUNT.toString());
      expect(bet.target).to.equal(TARGET);
      expect(bet.direction.under).to.not.be.undefined;
      expect(bet.resolved).to.be.false;
    });

    it("Resolves a dice bet (win)", async () => {
      // Mock VRF result: roll = 25 (under 50, so player wins)
      const vrfResult = Buffer.alloc(32);
      // Set bytes to produce roll = 25
      vrfResult.writeUInt32LE(25, 0);

      const tx = await program.methods
        .resolveDiceBet(Array.from(vrfResult))
        .accounts({
          vault: vaultPda,
          bet: betPda,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      console.log("Resolve dice bet transaction:", tx);

      const bet = await program.account.diceBet.fetch(betPda);
      expect(bet.resolved).to.be.true;
      expect(bet.randomValue.toNumber()).to.equal(25);
      expect(bet.payout).to.not.be.null;
      expect(bet.payout.toNumber()).to.be.greaterThan(0);
    });

    it("Places a dice bet (over)", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      const [newBetPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("dice_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      // Transfer SOL for bet
      const transferTx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: player.publicKey,
          lamports: BET_AMOUNT.toNumber(),
        })
      );
      await provider.sendAndConfirm(transferTx);

      await program.methods
        .placeDiceBetSol(TARGET, { over: {} }, BET_AMOUNT)
        .accounts({
          vault: vaultPda,
          bet: newBetPda,
          player: player.publicKey,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([player])
        .rpc();

      // Resolve with losing roll (25 < 50, so "over" loses)
      const vrfResult = Buffer.alloc(32);
      vrfResult.writeUInt32LE(25, 0);

      await program.methods
        .resolveDiceBet(Array.from(vrfResult))
        .accounts({
          vault: vaultPda,
          bet: newBetPda,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
        })
        .rpc();

      const bet = await program.account.diceBet.fetch(newBetPda);
      expect(bet.resolved).to.be.true;
      expect(bet.payout).to.be.null; // Player lost
    });
  });

  describe("Plinko Game (SOL)", () => {
    let betPda: PublicKey;
    const ROWS = 8;
    const BET_AMOUNT = new anchor.BN(0.1 * LAMPORTS_PER_SOL);

    it("Places a Plinko bet", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      [betPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("plinko_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      // Transfer SOL for bet
      const transferTx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: player.publicKey,
          lamports: BET_AMOUNT.toNumber(),
        })
      );
      await provider.sendAndConfirm(transferTx);

      const tx = await program.methods
        .placePlinkoBetSol(ROWS, BET_AMOUNT)
        .accounts({
          vault: vaultPda,
          bet: betPda,
          player: player.publicKey,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([player])
        .rpc();

      console.log("Plinko bet transaction:", tx);

      const bet = await program.account.plinkoBet.fetch(betPda);
      expect(bet.player.toString()).to.equal(player.publicKey.toString());
      expect(bet.amount.toString()).to.equal(BET_AMOUNT.toString());
      expect(bet.rows).to.equal(ROWS);
      expect(bet.resolved).to.be.false;
    });

    it("Resolves a Plinko bet", async () => {
      // Mock VRF result
      const vrfResult = Buffer.alloc(32);
      vrfResult.fill(0x12); // Some random bytes

      const tx = await program.methods
        .resolvePlinkoBet(Array.from(vrfResult))
        .accounts({
          vault: vaultPda,
          bet: betPda,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();

      console.log("Resolve Plinko bet transaction:", tx);

      const bet = await program.account.plinkoBet.fetch(betPda);
      expect(bet.resolved).to.be.true;
      expect(bet.path).to.not.be.null;
      expect(bet.multiplier).to.not.be.null;
      expect(bet.payout).to.not.be.null;
    });

    it("Fails with invalid rows", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      const [invalidBetPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("plinko_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      try {
        await program.methods
          .placePlinkoBetSol(5, BET_AMOUNT) // Invalid: < 8
          .accounts({
            vault: vaultPda,
            bet: invalidBetPda,
            player: player.publicKey,
            playerAta: player.publicKey,
            vaultAta: vaultPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([player])
          .rpc();
        
        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err.toString()).to.include("InvalidPlinkoRows");
      }
    });
  });

  describe("Admin Functions", () => {
    it("Updates house edge", async () => {
      const newHouseEdge = 300; // 3%

      const tx = await program.methods
        .updateHouseEdge(newHouseEdge)
        .accounts({
          vault: vaultPda,
          admin: admin.publicKey,
        })
        .rpc();

      console.log("Update house edge transaction:", tx);

      const vault = await program.account.casinoVault.fetch(vaultPda);
      expect(vault.houseEdgeBps).to.equal(newHouseEdge);
    });

    it("Updates bet limits", async () => {
      const newMinBet = new anchor.BN(50000);
      const newMaxBet = new anchor.BN(20 * LAMPORTS_PER_SOL);

      const tx = await program.methods
        .updateBetLimits(newMinBet, newMaxBet)
        .accounts({
          vault: vaultPda,
          admin: admin.publicKey,
        })
        .rpc();

      console.log("Update bet limits transaction:", tx);

      const vault = await program.account.casinoVault.fetch(vaultPda);
      expect(vault.minBet.toString()).to.equal(newMinBet.toString());
      expect(vault.maxBet.toString()).to.equal(newMaxBet.toString());
    });

    it("Fails when non-admin tries to update", async () => {
      try {
        await program.methods
          .updateHouseEdge(250)
          .accounts({
            vault: vaultPda,
            admin: player.publicKey, // Not admin
          })
          .signers([player])
          .rpc();
        
        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err.toString()).to.include("Unauthorized");
      }
    });
  });

  describe("Edge Cases", () => {
    it("Fails with bet below minimum", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("dice_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      try {
        await program.methods
          .placeDiceBetSol(50, { under: {} }, new anchor.BN(1000)) // Below minimum
          .accounts({
            vault: vaultPda,
            bet: betPda,
            player: player.publicKey,
            playerAta: player.publicKey,
            vaultAta: vaultPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([player])
          .rpc();
        
        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err.toString()).to.include("BetBelowMinimum");
      }
    });

    it("Fails to resolve already resolved bet", async () => {
      const timestamp = new anchor.BN(Date.now() / 1000);
      const [betPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("dice_bet"),
          player.publicKey.toBuffer(),
          timestamp.toArrayLike(Buffer, "le", 8),
        ],
        program.programId
      );

      // Place and resolve bet
      const transferTx = new Transaction().add(
        SystemProgram.transfer({
          fromPubkey: admin.publicKey,
          toPubkey: player.publicKey,
          lamports: MIN_BET.toNumber(),
        })
      );
      await provider.sendAndConfirm(transferTx);

      await program.methods
        .placeDiceBetSol(50, { under: {} }, MIN_BET)
        .accounts({
          vault: vaultPda,
          bet: betPda,
          player: player.publicKey,
          playerAta: player.publicKey,
          vaultAta: vaultPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([player])
        .rpc();

      const vrfResult = Buffer.alloc(32);
      vrfResult.writeUInt32LE(25, 0);

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

      // Try to resolve again
      try {
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
        
        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err.toString()).to.include("BetAlreadyResolved");
      }
    });
  });
});
