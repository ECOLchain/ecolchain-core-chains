use anchor_lang::prelude::*;

use crate::{
    constants::{CARENCIA_RETOMADA_S, SEED_GLOBAL, SEED_LOTE},
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

pub fn handle_parte_open_disputa(
    ctx: Context<ParteOpenDisputa>,
    motivo: MotivoDisputa,
) -> Result<()> {
    ctx.accounts.config.exigir_disputas_liberadas()?;
    // Prazo expirado só pelo crank, que confere o relógio.
    require!(
        motivo != MotivoDisputa::PrazoExpirado,
        EcolLoteError::ParametroInvalido
    );
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.parte.key();
    let lote = &mut ctx.accounts.lote;
    // Sem isso, uma parte reabriria a disputa logo após o "prosseguir" e travaria o escrow para sempre.
    let etapa = lote.estado.etapa_disputavel();
    require!(
        etapa.is_none() || etapa != lote.etapa_julgada,
        EcolLoteError::DisputaJaJulgada
    );
    let para = lote.estado.abrir_disputa(motivo)?;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado {
        lote: lote_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
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

/// Só em modo Normal: com retirada e recebimento bloqueados, o atraso não é culpa das partes.
/// Depois de uma pausa, há carência para as partes retomarem antes do crank derrubar os lotes.
pub fn handle_public_crank_prazo_expirado(ctx: Context<PublicCrankPrazoExpirado>) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    let agora = Clock::get()?.unix_timestamp;
    let fim_carencia = config
        .retomado_em
        .checked_add(CARENCIA_RETOMADA_S)
        .ok_or(EcolLoteError::Overflow)?;
    require!(agora >= fim_carencia, EcolLoteError::EmCarencia);
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cranker.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.expirar_prazo(agora, lote.prazo_entrega)?;
    let (de, seq) = lote.transicionar(para, agora)?;
    emit_cpi!(LoteTransicionado {
        lote: lote_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecisaoDisputa {
    /// Volta para a etapa de origem, opcionalmente com o peso reclassificado. As partes
    /// não podem reabrir disputa nessa mesma etapa depois disso.
    Prosseguir { novo_peso_g: Option<u64> },
    /// Só para disputa aberta no recebimento: encerra a operação liberando `valor_liberado_centavos`
    /// à cooperativa e reembolsando o restante à indústria. O intermediador co-assina.
    Liberar {
        valor_liberado_centavos: u64,
        novo_peso_g: Option<u64>,
        ref_hash: [u8; 32],
    },
    /// Encerra o lote; o intermediador co-assina atestando a devolução do escrow à indústria.
    Reembolsar { ref_hash: [u8; 32] },
}

#[event_cpi]
#[derive(Accounts)]
pub struct ArbitroResolveDisputa<'info> {
    pub arbitro: Signer<'info>,
    /// Obrigatório quando a decisão movimenta o escrow (liberação ou reembolso).
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
            lote.etapa_julgada = para.etapa_disputavel();
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
            emit_cpi!(LoteTransicionado {
                lote: lote_key,
                seq,
                de,
                para,
                ator,
                ts: agora
            });
            if let (Some(peso_novo_g), Some(seq)) = (novo_peso_g, seq_peso) {
                emit_cpi!(LotePesoAlterado {
                    lote: lote_key,
                    seq,
                    peso_anterior_g,
                    peso_novo_g,
                    ator
                });
            }
        }
        DecisaoDisputa::Liberar {
            valor_liberado_centavos,
            novo_peso_g,
            ref_hash,
        } => {
            exigir_intermediador(&ctx.accounts.intermediador, config)?;
            require!(ref_hash != [0; 32], EcolLoteError::ReferenciaVazia);
            let lote = &mut ctx.accounts.lote;
            let para = lote.estado.resolver_liberacao()?;
            let reembolso_centavos = lote
                .valor_centavos
                .checked_sub(valor_liberado_centavos)
                .ok_or(EcolLoteError::ValorLiberadoInvalido)?;
            let peso_anterior_g = lote.peso_g;
            if let Some(novo) = novo_peso_g {
                require!(novo > 0, EcolLoteError::PesoInvalido);
                lote.peso_g = novo;
            }
            lote.valor_liberado_centavos = valor_liberado_centavos;
            let (de, seq) = lote.transicionar(para, agora)?;
            let seq_peso = match novo_peso_g {
                Some(_) => Some(lote.proximo_seq()?),
                None => None,
            };
            let seq_liberacao = lote.proximo_seq()?;
            let seq_reembolso = match reembolso_centavos {
                0 => None,
                _ => Some(lote.proximo_seq()?),
            };
            emit_cpi!(LoteTransicionado {
                lote: lote_key,
                seq,
                de,
                para,
                ator,
                ts: agora
            });
            if let (Some(peso_novo_g), Some(seq)) = (novo_peso_g, seq_peso) {
                emit_cpi!(LotePesoAlterado {
                    lote: lote_key,
                    seq,
                    peso_anterior_g,
                    peso_novo_g,
                    ator
                });
            }
            emit_cpi!(EscrowMovimentado {
                lote: lote_key,
                seq: seq_liberacao,
                movimento: MovimentoEscrow::Liberacao,
                ref_hash,
                valor_centavos: valor_liberado_centavos,
            });
            if let Some(seq) = seq_reembolso {
                emit_cpi!(EscrowMovimentado {
                    lote: lote_key,
                    seq,
                    movimento: MovimentoEscrow::Reembolso,
                    ref_hash,
                    valor_centavos: reembolso_centavos,
                });
            }
        }
        DecisaoDisputa::Reembolsar { ref_hash } => {
            exigir_intermediador(&ctx.accounts.intermediador, config)?;
            require!(ref_hash != [0; 32], EcolLoteError::ReferenciaVazia);
            let lote = &mut ctx.accounts.lote;
            let para = lote.estado.resolver_reembolso()?;
            let valor_centavos = lote.valor_centavos;
            let (de, seq) = lote.transicionar(para, agora)?;
            let seq_escrow = lote.proximo_seq()?;
            emit_cpi!(LoteTransicionado {
                lote: lote_key,
                seq,
                de,
                para,
                ator,
                ts: agora
            });
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

fn exigir_intermediador(intermediador: &Option<Signer>, config: &GlobalConfig) -> Result<()> {
    let intermediador = intermediador
        .as_ref()
        .ok_or(EcolLoteError::NaoEhIntermediador)?;
    require_keys_eq!(
        intermediador.key(),
        config.intermediador,
        EcolLoteError::NaoEhIntermediador
    );
    Ok(())
}
