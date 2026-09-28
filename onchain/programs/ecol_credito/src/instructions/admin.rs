use anchor_lang::prelude::*;

use crate::{
    constants::SEED_CONFIG, error::EcolCreditoError, events::CreditoConfigAtualizada,
    program::EcolCredito, state::CreditoConfig,
};

impl CreditoConfig {
    fn evento(&self) -> CreditoConfigAtualizada {
        CreditoConfigAtualizada {
            admin: self.admin,
            admin_pendente: self.admin_pendente,
            operador: self.operador,
            registrador: self.registrador,
            intermediador: self.intermediador,
            pausado: self.pausado,
        }
    }
}

/// Cria a configuração. Exige a upgrade authority para ninguém se adiantar ao deploy.
#[event_cpi]
#[derive(Accounts)]
pub struct AdminInitConfig<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub upgrade_authority: Signer<'info>,
    #[account(init, payer = payer, space = 8 + CreditoConfig::INIT_SPACE, seeds = [SEED_CONFIG], bump)]
    pub config: Account<'info, CreditoConfig>,
    #[account(constraint = este_programa.programdata_address()? == Some(program_data.key()))]
    pub este_programa: Program<'info, EcolCredito>,
    #[account(
        constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key())
            @ EcolCreditoError::NaoEhUpgradeAuthority
    )]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

pub fn handle_admin_init_config(
    ctx: Context<AdminInitConfig>,
    admin: Pubkey,
    operador: Pubkey,
    registrador: Pubkey,
    intermediador: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = admin;
    config.operador = operador;
    config.registrador = registrador;
    config.intermediador = intermediador;
    config.validar_chaves()?;
    config.bump = ctx.bumps.config;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct AdminUpdateConfig<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_CONFIG], bump = config.bump, has_one = admin @ EcolCreditoError::NaoEhAdmin)]
    pub config: Account<'info, CreditoConfig>,
}

pub fn handle_admin_update_config(
    ctx: Context<AdminUpdateConfig>,
    operador: Pubkey,
    registrador: Pubkey,
    intermediador: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.operador = operador;
    config.registrador = registrador;
    config.intermediador = intermediador;
    config.validar_chaves()?;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

pub fn handle_admin_set_pausado(ctx: Context<AdminUpdateConfig>, pausado: bool) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.pausado = pausado;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Passo 1 da troca de admin; `Pubkey::default()` cancela.
pub fn handle_admin_propose_admin(
    ctx: Context<AdminUpdateConfig>,
    novo_admin: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        novo_admin == Pubkey::default() || !config.eh_chave_de_papel(&novo_admin),
        EcolCreditoError::ChavesDeConfigInvalidas
    );
    config.admin_pendente = novo_admin;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct AdminAcceptAdmin<'info> {
    pub novo_admin: Signer<'info>,
    #[account(
        mut,
        seeds = [SEED_CONFIG],
        bump = config.bump,
        constraint = config.admin_pendente == novo_admin.key() @ EcolCreditoError::SemAdminPendente
    )]
    pub config: Account<'info, CreditoConfig>,
}

pub fn handle_admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.novo_admin.key();
    config.admin_pendente = Pubkey::default();
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}
