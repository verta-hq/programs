use anchor_lang::prelude::*;

declare_id!("RFNuLuqbuSJ5JcPpcjtLumvrRDQK8ttn5rbSgzxrpQR");

#[program]
pub mod royalties_private {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
