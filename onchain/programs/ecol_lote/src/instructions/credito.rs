use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_LOTE},
    error::EcolLoteError,
    events::LoteTransicionado,
    state::{GlobalConfig, Lote},
};

/// Chamada via CPI pelo programa `ecol_credito`, que assina com a PDA registrada em `credito_autoridade`.
#[event_cpi]
#[derive(Accounts)]
pub struct CreditoMarkAgregado<'info> {
    pub credito_autoridade: Signer<'info>,
    #[account(
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = credito_autoridade @ EcolLoteError::NaoEhAutoridadeCredito
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_credito_mark_agregado(ctx: Context<CreditoMarkAgregado>, credito: Pubkey) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.credito_autoridade.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.agregar()?;
    lote.credito = credito;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    Ok(())
}
