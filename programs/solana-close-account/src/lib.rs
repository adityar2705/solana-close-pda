use anchor_lang::__private::CLOSED_ACCOUNT_DISCRIMINATOR;
use anchor_lang::prelude::*;
use anchor_spl::token::{mint_to, Mint, MintTo, Token, TokenAccount};
use std::ops::DerefMut;

declare_id!("BQJWKYgRi1LaJMUtcGHLPELxFhsgH5UHPZX4rG7Hnw5H");

#[program]
pub mod solana_close_account {
    use super::*;

    //function to enter the lottery
    pub fn enter_lottery(ctx : Context<EnterLottery>) -> Result<()>{
        msg!("Initializing lottery entry.");

        ctx.accounts.lottery_entry.timestamp = Clock::get().unwrap().unix_timestamp;
        ctx.accounts.lottery_entry.user = ctx.accounts.user.key();
        ctx.accounts.lottery_entry.user_ata = ctx.accounts.user_ata.key();
        ctx.accounts.lottery_entry.bump = *ctx.bumps.get("lottery_entry").unwrap();

        msg!("Entry initialized!");
        Ok(())
    }

    //function to redeem the winnings of the lottery
    pub fn redeem_winnings_insecure(ctx : Context<RedeemWinnings>) ->Result<()>{
        msg!("Calculating the winnings of the lottery account.");
        let amount = ctx.accounts.lottery_entry.timestamp as u64 * 10;

        msg!("Minting {} tokens in rewards", amount);

        //program signer seeds
        let auth_bump = *ctx.bumps.get("mint_auth").unwrap();
        let auth_seeds = &[MINT_SEED.as_bytes(), &[auth_bump]];
        let signer = &[&auth_seeds[..]];

        //donate the minted tokens to the user ata
        mint_to(ctx.accounts.mint_ctx().with_signer(signer), amount)?;
        
        msg!("Closing account...");
        let account_to_close = ctx.accounts.lottery_entry.to_account_info();
        let dest_starting_lamports = ctx.accounts.user.lamports();
        
        **ctx.accounts.user.lamports.borrow_mut() = dest_starting_lamports.checked_add(account_to_close.lamports()).unwrap();
        **account_to_close.lamports.borrow_mut() = 0;

        //setting every byte of the data in the Lottery PDA as 0
        let mut data = account_to_close.try_borrow_mut_data()?;

        //going through each byte of the borrowed data
        for byte in data.deref_mut().iter_mut() {
            *byte = 0;
        }

        msg!("Lottery lamports: {:?}", account_to_close.lamports);
        msg!("Lottery account closed");

        Ok(())
    }

    //function to force defund the lamports
    pub fn force_defund(ctx :Context<ForceDefund>) -> Result<()>{
        //we want to force drain the lottery PDA -> so we fetch the account
        let account = &ctx.accounts.data_account;

        //bytes > 8 so account has been initialized
        msg!("Checking validity of the data.");
        let data = account.try_borrow_data()?;
        assert!(data.len() > 8);

        //creating our custom discriminator -> 0u8 -> 0 in u8 format
        let mut discriminator = [0u8 ; 8];
        discriminator.copy_from_slice(&data[0..8]);

        //comparing with Anchor's in-built closed account discriminator
        if discriminator != CLOSED_ACCOUNT_DISCRIMINATOR{
            return err!(MintError::InvalidDiscriminator);
        }

        //storing the initial amount for further use
        msg!("Transferring the tokens to destination.");
        let dest_starting_lamports = ctx.accounts.destination.lamports();

        //setting account lamports to 0 and sending the lamports to destination
        **ctx.accounts.destination.lamports.borrow_mut() = dest_starting_lamports.checked_add(account.lamports()).unwrap();
        **account.lamports.borrow_mut() = 0;

        Ok(())
    }

    //function to securely redeeeming the tokens
    pub fn redeem_winnings_secure(ctx: Context<RedeemWinningsSecure>) -> Result<()> {
        msg!("Calculating winnings");
        let amount = ctx.accounts.lottery_entry.timestamp as u64 * 10;
     
        msg!("Minting {} tokens in rewards", amount);
        // program signer seeds
        let auth_bump = *ctx.bumps.get("mint_auth").unwrap();

        //since it is a specific bump we need to include that in the signers as well
        let auth_seeds = &[MINT_SEED.as_bytes(), &[auth_bump]];
        let signer = &[&auth_seeds[..]];
     
        // redeem rewards by minting to user
        mint_to(ctx.accounts.mint_ctx().with_signer(signer), amount)?;
     
        Ok(())
    }
}

//account for the enter lottery instruction
#[derive(Accounts)]
pub struct EnterLottery<'info>{
    #[account(
        init,
        seeds = [user.key().as_ref()],
        bump,
        payer = user,
        space = 8 + 1 + 32 + 1 + 8 + 32
    )]
    pub lottery_entry : Account<'info, LotteryAccount>,

    #[account(mut)]
    pub user : Signer<'info>,

    //the destination to send the minted tokens
    pub user_ata : Account<'info, TokenAccount>,
    pub system_program : Program<'info, System>,
}

//account for the redeem winnings instruction
#[derive(Accounts)]
pub struct RedeemWinnings<'info>{
    //program expects this account to be initialized
    #[account(
        mut,
        seeds = [user.key().as_ref()],
        bump = lottery_entry.bump,
        has_one = user
    )]
    pub lottery_entry : Account<'info, LotteryAccount>,
    #[account(mut)]
    pub user : Signer<'info>,

    //get the user's associated token account
    #[account(
        mut,
        constraint = user_ata.key() == lottery_entry.user_ata
    )]
    pub user_ata : Account<'info, TokenAccount>,

    //getting the reward token mint account
    #[account(
        mut,
        constraint = reward_mint.key() == user_ata.mint
    )]
    pub reward_mint: Account<'info, Mint>,

    ///CHECK: mint authority
    #[account(
        seeds = [MINT_SEED.as_bytes()],
        bump
    )]
    pub mint_auth: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}

//making a secure redeem winnings context
#[derive(Accounts)]
pub struct RedeemWinningsSecure<'info>{
    //program expects this to be initialized
    #[account(
        mut, 
        seeds = [user.key().as_ref()],
        bump = lottery_entry.bump,
        has_one = user,

        //tells Anchor to transfer lamports to user on closing the account -> rest of the struct will be the exact same
        close = user
    )]
    pub lottery_entry: Account<'info, LotteryAccount>,

    //getting the user as a signer
    #[account(mut)]
    pub user: Signer<'info>,

    //getting the user associated token account
    #[account(
        mut,
        constraint = user_ata.key() == lottery_entry.user_ata
    )]
    pub user_ata: Account<'info, TokenAccount>,

    //getting the token reward mint
    #[account(
        mut,
        constraint = reward_mint.key() == user_ata.mint
    )]
    pub reward_mint: Account<'info, Mint>,

    ///CHECK: mint authority
    #[account(
        seeds = [MINT_SEED.as_bytes()],
        bump
    )]
    pub mint_auth: AccountInfo<'info>,
    pub token_program: Program<'info, Token>
}

//implementing the mint context for the above struct
impl<'info> RedeemWinningsSecure<'info>{
    pub fn mint_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>>{
        let cpi_program = self.token_program.to_account_info();

        //the necessary accounts for the mint context
        let cpi_accounts = MintTo{
            mint : self.reward_mint.to_account_info(),
            to : self.user_ata.to_account_info(),
            authority : self.mint_auth.to_account_info()
        };

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

//account for force defund instruction
#[derive(Accounts)]
pub struct ForceDefund<'info>{
    ///CHECK: safe
    #[account(mut)]
    data_account : AccountInfo<'info>,
    ///CHECK: safe
    #[account(mut)]
    destination: AccountInfo<'info>,
}

//account for our user's lottery state
#[account]
pub struct LotteryAccount{
    is_initialized : bool,
    user : Pubkey,
    bump: u8,
    timestamp: i64,
    user_ata: Pubkey,
}

pub const MINT_SEED : &str = "mint-seed";

//implementing the mint context for redeem winnings -> completely self written
impl<'info> RedeemWinnings<'info>{
    pub fn mint_ctx(&self) ->CpiContext<'_, '_, '_, 'info, MintTo<'info>>{
        let cpi_program = self.token_program.to_account_info();

        //listing the accounts needed for the mint instruction
        let cpi_accounts = MintTo{
            mint : self.reward_mint.to_account_info(),
            to : self.user_ata.to_account_info(),
            authority : self.mint_auth.to_account_info()
        };

        //creating the new context for the instruction
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

//making our error code
#[error_code]
pub enum MintError{
    #[msg("Expected closed account discriminator")]
    InvalidDiscriminator
}
