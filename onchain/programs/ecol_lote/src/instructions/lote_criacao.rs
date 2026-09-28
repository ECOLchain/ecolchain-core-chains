use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::LoteCriado,
    state::{EstadoLote, GlobalConfig, Lote, Material, Papel, Participante},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(lote_id: u64)]
pub struct CooperativaCreateLote<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Account<'info, Participante>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        init,
        payer = payer,
        space = 8 + Lote::INIT_SPACE,
        seeds = [SEED_LOTE, cooperativa.key().as_ref(), &lote_id.to_le_bytes()],
        bump
    )]
    pub lote: Account<'info, Lote>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cooperativa_create_lote(
    ctx: Context<CooperativaCreateLote>,
    lote_id: u64,
    material: Material,
    peso_g: u64,
    coletor_ref: [u8; 32],
    evidencias_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    require!(peso_g > 0, EcolLoteError::PesoInvalido);
    require!(coletor_ref != [0; 32], EcolLoteError::ReferenciaVazia);
    require!(evidencias_hash != [0; 32], EcolLoteError::ReferenciaVazia);

    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let cooperativa = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    lote.cooperativa = cooperativa;
    lote.lote_id = lote_id;
    lote.material = material;
    lote.peso_g = peso_g;
    lote.coletor_ref = coletor_ref;
    lote.evidencias_hash = evidencias_hash;
    lote.estado = EstadoLote::Criado;
    lote.criado_em = agora;
    lote.atualizado_em = agora;
    lote.bump = ctx.bumps.lote;
    let seq = lote.proximo_seq()?;

    emit_cpi!(LoteCriado {
        lote: lote_key,
        seq,
        cooperativa,
        lote_id,
        material,
        peso_g,
        coletor_ref,
        evidencias_hash,
        ts: agora,
    });
    Ok(())
}
