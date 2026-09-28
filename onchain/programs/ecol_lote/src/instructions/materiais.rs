use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_MATERIAL},
    error::EcolLoteError,
    events::MaterialAtualizado,
    state::{GlobalConfig, Material},
};

/// Cadastra um tipo de material. O código é escolhido pelo operador e não muda depois.
#[event_cpi]
#[derive(Accounts)]
#[instruction(codigo: u16)]
pub struct OperadorCreateMaterial<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        init,
        payer = payer,
        space = 8 + Material::INIT_SPACE,
        seeds = [SEED_MATERIAL, &codigo.to_le_bytes()],
        bump
    )]
    pub material: Account<'info, Material>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_create_material(
    ctx: Context<OperadorCreateMaterial>,
    codigo: u16,
    nome: String,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    Material::validar_nome(&nome)?;
    let material = &mut ctx.accounts.material;
    material.codigo = codigo;
    material.nome = nome;
    material.ativo = true;
    material.bump = ctx.bumps.material;
    emit_cpi!(MaterialAtualizado {
        material: material.key(),
        codigo,
        nome: material.nome.clone(),
        ativo: true,
    });
    Ok(())
}

/// Renomeia, ativa ou desativa um material. O código não muda.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorUpdateMaterial<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_MATERIAL, &material.codigo.to_le_bytes()],
        bump = material.bump
    )]
    pub material: Account<'info, Material>,
}

pub fn handle_operador_update_material(
    ctx: Context<OperadorUpdateMaterial>,
    nome: String,
    ativo: bool,
) -> Result<()> {
    Material::validar_nome(&nome)?;
    let material = &mut ctx.accounts.material;
    material.nome = nome;
    material.ativo = ativo;
    emit_cpi!(MaterialAtualizado {
        material: material.key(),
        codigo: material.codigo,
        nome: material.nome.clone(),
        ativo,
    });
    Ok(())
}
