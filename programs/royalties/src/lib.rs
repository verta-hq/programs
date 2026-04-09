use anchor_lang::prelude::*;

declare_id!("HU9MebBjowZV3Hyr3GagXM2Pzx3xNdwbQbZqsJK3ZMcC");

#[program]
pub mod royalties {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
