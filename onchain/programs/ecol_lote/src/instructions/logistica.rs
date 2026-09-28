use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::{EscrowMovimentado, LoteTransicionado, MovimentoEscrow},
    state::{GlobalConfig, Lote, Papel, Participante},
};

/// Retirada do lote: transportador e cooperativa assinam juntos a passagem de custódia.
#[event_cpi]
#[derive(Accounts)]
pub struct TransportadorPickupLote<'info> {
    pub transportador: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, transportador.key().as_ref()], bump = transportador_part.bump)]
    pub transportador_part: Account<'info, Participante>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = cooperativa @ EcolLoteError::NaoEhCooperativaDoLote
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_transportador_pickup_lote(ctx: Context<TransportadorPickupLote>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.transportador_part.exigir(Papel::Transportador)?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.transportador.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.iniciar_transporte()?;
    lote.transportador = ator;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    Ok(())
}

/// Indústria confirma o recebimento. Se o peso divergir além da tolerância, o lote vai direto para disputa.
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaConfirmRecebimento<'info> {
    pub industria: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = industria @ EcolLoteError::NaoEhIndustriaDoLote
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_industria_confirm_recebimento(
    ctx: Context<IndustriaConfirmRecebimento>,
    peso_recebido_g: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    require!(peso_recebido_g > 0, EcolLoteError::PesoInvalido);
    let tolerancia = config.tolerancia_peso_bps;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.industria.key();
    let lote = &mut ctx.accounts.lote;
    let divergente = lote.peso_diverge(peso_recebido_g, tolerancia)?;
    let para = lote.estado.receber(divergente)?;
    lote.peso_recebido_g = peso_recebido_g;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    Ok(())
}

/// O intermediador atesta que liberou o escrow em R$ para a cooperativa.
#[event_cpi]
#[derive(Accounts)]
pub struct IntermediadorConfirmLiberacao<'info> {
    pub intermediador: Signer<'info>,
    #[account(
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = intermediador @ EcolLoteError::NaoEhIntermediador
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_intermediador_confirm_liberacao(
    ctx: Context<IntermediadorConfirmLiberacao>,
    liberacao_ref_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    require!(liberacao_ref_hash != [0; 32], EcolLoteError::ReferenciaVazia);
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.intermediador.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.liberar_escrow()?;
    let valor_centavos = lote.valor_centavos;
    let (de, seq) = lote.transicionar(para, agora)?;
    let seq_escrow = lote.proximo_seq()?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    emit_cpi!(EscrowMovimentado {
        lote: lote_key,
        seq: seq_escrow,
        movimento: MovimentoEscrow::Liberacao,
        ref_hash: liberacao_ref_hash,
        valor_centavos,
    });
    Ok(())
}
