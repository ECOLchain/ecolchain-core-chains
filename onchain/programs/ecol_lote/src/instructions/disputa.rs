use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_GLOBAL, SEED_LOTE},
    error::EcolLoteError,
    events::{EscrowMovimentado, LotePesoAlterado, LoteTransicionado, MovimentoEscrow},
    state::{EstadoLote, GlobalConfig, Lote, MotivoDisputa},
};

/// Qualquer parte do lote (cooperativa, indústria, transportador) pode abrir disputa.
#[event_cpi]
#[derive(Accounts)]
pub struct ParteOpenDisputa<'info> {
    pub parte: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        constraint = lote.eh_parte(&parte.key()) @ EcolLoteError::NaoEhParteDoLote
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_parte_open_disputa(ctx: Context<ParteOpenDisputa>, motivo: MotivoDisputa) -> Result<()> {
    ctx.accounts.config.exigir_disputas_liberadas()?;
    // Prazo expirado só pelo crank, que confere o relógio.
    require!(motivo != MotivoDisputa::PrazoExpirado, EcolLoteError::ParametroInvalido);
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.parte.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.abrir_disputa(motivo)?;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    Ok(())
}

/// Crank permissionless: lote vendido ou em transporte além do prazo de entrega entra em disputa.
#[event_cpi]
#[derive(Accounts)]
pub struct PublicCrankPrazoExpirado<'info> {
    pub cranker: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_public_crank_prazo_expirado(ctx: Context<PublicCrankPrazoExpirado>) -> Result<()> {
    ctx.accounts.config.exigir_disputas_liberadas()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cranker.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.expirar_prazo(agora, lote.prazo_entrega)?;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecisaoDisputa {
    /// Volta para a etapa de origem, opcionalmente com o peso reclassificado.
    Prosseguir { novo_peso_g: Option<u64> },
    /// Encerra o lote; o intermediador co-assina atestando a devolução do escrow à indústria.
    Reembolsar { ref_hash: [u8; 32] },
}

#[event_cpi]
#[derive(Accounts)]
pub struct ArbitroResolveDisputa<'info> {
    pub arbitro: Signer<'info>,
    /// Obrigatório só no reembolso.
    pub intermediador: Option<Signer<'info>>,
    #[account(
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = arbitro @ EcolLoteError::NaoEhArbitro
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_arbitro_resolve_disputa(
    ctx: Context<ArbitroResolveDisputa>,
    decisao: DecisaoDisputa,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_disputas_liberadas()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.arbitro.key();

    match decisao {
        DecisaoDisputa::Prosseguir { novo_peso_g } => {
            let prazo_entrega_s = config.prazo_entrega_s;
            let lote = &mut ctx.accounts.lote;
            let para = lote.estado.resolver_prosseguir()?;
            // Sem prazo novo, o crank devolveria o lote à disputa logo em seguida.
            if matches!(para, EstadoLote::Vendido | EstadoLote::EmTransporte) {
                lote.prazo_entrega = agora
                    .checked_add(prazo_entrega_s)
                    .ok_or(EcolLoteError::Overflow)?;
            }
            let peso_anterior_g = lote.peso_g;
            if let Some(novo) = novo_peso_g {
                require!(novo > 0, EcolLoteError::PesoInvalido);
                lote.peso_g = novo;
            }
            let (de, seq) = lote.transicionar(para, agora)?;
            let seq_peso = match novo_peso_g {
                Some(_) => Some(lote.proximo_seq()?),
                None => None,
            };
            emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
            if let (Some(peso_novo_g), Some(seq)) = (novo_peso_g, seq_peso) {
                emit_cpi!(LotePesoAlterado { lote: lote_key, seq, peso_anterior_g, peso_novo_g, ator });
            }
        }
        DecisaoDisputa::Reembolsar { ref_hash } => {
            let intermediador = ctx
                .accounts
                .intermediador
                .as_ref()
                .ok_or(EcolLoteError::NaoEhIntermediador)?;
            require_keys_eq!(
                intermediador.key(),
                config.intermediador,
                EcolLoteError::NaoEhIntermediador
            );
            require!(ref_hash != [0; 32], EcolLoteError::ReferenciaVazia);
            let lote = &mut ctx.accounts.lote;
            let para = lote.estado.resolver_reembolso()?;
            let valor_centavos = lote.valor_centavos;
            let (de, seq) = lote.transicionar(para, agora)?;
            let seq_escrow = lote.proximo_seq()?;
            emit_cpi!(LoteTransicionado { lote: lote_key, seq, de, para, ator, ts: agora });
            emit_cpi!(EscrowMovimentado {
                lote: lote_key,
                seq: seq_escrow,
                movimento: MovimentoEscrow::Reembolso,
                ref_hash,
                valor_centavos,
            });
        }
    }
    Ok(())
}
