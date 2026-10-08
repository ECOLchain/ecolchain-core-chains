use anchor_lang::prelude::*;

use crate::{
    constants::{MAX_VARIACOES, SEED_GLOBAL, SEED_MATERIAL, SEED_VARIACAO},
    error::EcolLoteError,
    events::{MaterialAtualizado, MaterialGarrafasAtualizado, VariacaoAtualizada},
    state::{codificar_nome, GlobalConfig, Material, MaterialVariacao},
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

/// Cadastra a próxima variação do material (índice = `qtd_variacoes + 1`), como uma cor do vidro.
/// A partir da primeira, lotes novos do material exigem variação.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorCreateVariacao<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_MATERIAL, &material.codigo.to_le_bytes()],
        bump = material.bump
    )]
    pub material: Account<'info, Material>,
    #[account(
        init,
        payer = payer,
        space = 8 + MaterialVariacao::INIT_SPACE,
        seeds = [
            SEED_VARIACAO,
            &material.codigo.to_le_bytes(),
            &[material.qtd_variacoes.saturating_add(1)]
        ],
        bump
    )]
    pub variacao: Account<'info, MaterialVariacao>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_create_variacao(
    ctx: Context<OperadorCreateVariacao>,
    nome: String,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let nome = codificar_nome(&nome)?;
    let material = &mut ctx.accounts.material;
    require!(
        material.qtd_variacoes < MAX_VARIACOES,
        EcolLoteError::LimiteDeVariacoes
    );
    let indice = material
        .qtd_variacoes
        .checked_add(1)
        .ok_or(EcolLoteError::Overflow)?;
    material.qtd_variacoes = indice;
    let variacao = &mut ctx.accounts.variacao;
    variacao.material = material.codigo;
    variacao.indice = indice;
    variacao.nome = nome;
    variacao.ativa = true;
    variacao.bump = ctx.bumps.variacao;
    emit_cpi!(VariacaoAtualizada {
        variacao: variacao.key(),
        material: material.codigo,
        indice,
        nome,
        ativa: true,
    });
    Ok(())
}

/// Renomeia, ativa ou desativa uma variação. O índice não muda.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorUpdateVariacao<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_VARIACAO, &variacao.material.to_le_bytes(), &[variacao.indice]],
        bump = variacao.bump
    )]
    pub variacao: Account<'info, MaterialVariacao>,
}

pub fn handle_operador_update_variacao(
    ctx: Context<OperadorUpdateVariacao>,
    nome: String,
    ativa: bool,
) -> Result<()> {
    let variacao = &mut ctx.accounts.variacao;
    variacao.nome = codificar_nome(&nome)?;
    variacao.ativa = ativa;
    emit_cpi!(VariacaoAtualizada {
        variacao: variacao.key(),
        material: variacao.material,
        indice: variacao.indice,
        nome: variacao.nome,
        ativa,
    });
    Ok(())
}

/// Liga ou desliga a quantidade aproximada de garrafas nos lotes do material.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSetMaterialGarrafas<'info> {
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

pub fn handle_operador_set_material_garrafas(
    ctx: Context<OperadorSetMaterialGarrafas>,
    conta_garrafas: bool,
) -> Result<()> {
    let material = &mut ctx.accounts.material;
    material.conta_garrafas = conta_garrafas;
    emit_cpi!(MaterialGarrafasAtualizado {
        material: material.key(),
        codigo: material.codigo,
        conta_garrafas,
    });
    Ok(())
}
