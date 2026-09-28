use anchor_lang::prelude::*;

use crate::{
    constants::SEED_CONFIG,
    error::EcolReparticaoError,
    events::ReparticaoConfigAtualizada,
    program::EcolReparticao,
    state::{BpsReparticao, ReparticaoConfig},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ParamsReparticao {
    pub operador: Pubkey,
    pub zupy: Pubkey,
    pub intermediador: Pubkey,
    pub bps: BpsReparticao,
}

impl ReparticaoConfig {
    fn aplicar(&mut self, p: &ParamsReparticao) -> Result<()> {
        p.bps.validar()?;
        self.operador = p.operador;
        self.zupy = p.zupy;
        self.intermediador = p.intermediador;
        self.bps = p.bps;
        self.validar_chaves()
    }

    fn evento(&self) -> ReparticaoConfigAtualizada {
        ReparticaoConfigAtualizada {
            admin: self.admin,
            admin_pendente: self.admin_pendente,
            operador: self.operador,
            zupy: self.zupy,
            intermediador: self.intermediador,
            bps: self.bps,
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
    #[account(init, payer = payer, space = 8 + ReparticaoConfig::INIT_SPACE, seeds = [SEED_CONFIG], bump)]
    pub config: Account<'info, ReparticaoConfig>,
    #[account(constraint = este_programa.programdata_address()? == Some(program_data.key()))]
    pub este_programa: Program<'info, EcolReparticao>,
    #[account(
        constraint = program_data.upgrade_authority_address == Some(upgrade_authority.key())
            @ EcolReparticaoError::NaoEhUpgradeAuthority
    )]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

pub fn handle_admin_init_config(
    ctx: Context<AdminInitConfig>,
    admin: Pubkey,
    params: ParamsReparticao,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = admin;
    config.aplicar(&params)?;
    config.bump = ctx.bumps.config;
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}

/// Percentuais e chaves. Com o admin num Squads com timelock, toda mudança fica visível antes de valer.
#[event_cpi]
#[derive(Accounts)]
pub struct AdminUpdateConfig<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [SEED_CONFIG], bump = config.bump, has_one = admin @ EcolReparticaoError::NaoEhAdmin)]
    pub config: Account<'info, ReparticaoConfig>,
}

pub fn handle_admin_update_config(
    ctx: Context<AdminUpdateConfig>,
    params: ParamsReparticao,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.aplicar(&params)?;
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
        EcolReparticaoError::ChavesDeConfigInvalidas
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
        constraint = config.admin_pendente == novo_admin.key() @ EcolReparticaoError::SemAdminPendente
    )]
    pub config: Account<'info, ReparticaoConfig>,
}

pub fn handle_admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.novo_admin.key();
    config.admin_pendente = Pubkey::default();
    let evento = config.evento();
    emit_cpi!(evento);
    Ok(())
}
