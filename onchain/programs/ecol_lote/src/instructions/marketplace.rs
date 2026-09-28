use anchor_lang::prelude::*;

use crate::{
    constants::{DURACAO_LEILAO_MAX_S, SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::{LoteTransicionado, LoteVendido},
    state::{GlobalConfig, Lote, Papel, Participante},
};

/// Anuncia o lote (a partir de `Criado`) ou o reanuncia (a partir de `SemLance`) com novo preço mínimo.
/// As mesmas contas servem para `cooperativa_cancel_anuncio`.
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaListLote<'info> {
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Account<'info, Participante>,
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

pub fn handle_cooperativa_list_lote(
    ctx: Context<CooperativaListLote>,
    prazo_leilao: i64,
    preco_minimo_centavos: u64,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    // Cooperativa desativada não assume novos compromissos comerciais.
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    require!(preco_minimo_centavos > 0, EcolLoteError::ParametroInvalido);
    let agora = Clock::get()?.unix_timestamp;
    let limite = agora
        .checked_add(DURACAO_LEILAO_MAX_S)
        .ok_or(EcolLoteError::Overflow)?;
    require!(prazo_leilao <= limite, EcolLoteError::PrazoInvalido);
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.anunciar(agora, prazo_leilao)?;
    lote.preco_minimo_centavos = preco_minimo_centavos;
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

/// Retira o anúncio (para reprecificar ou desistir); o lote volta a poder ser anunciado.
pub fn handle_cooperativa_cancel_anuncio(ctx: Context<CooperativaListLote>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.cancelar_anuncio()?;
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

#[event_cpi]
#[derive(Accounts)]
pub struct OperadorCloseLeilaoSemLance<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump, has_one = operador @ EcolLoteError::NaoEhOperador)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_operador_close_leilao_sem_lance(
    ctx: Context<OperadorCloseLeilaoSemLance>,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.operador.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.encerrar_sem_lance(agora)?;
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

/// Registra o lance vencedor. Assinam juntos: a indústria (compradora), o operador (que apurou o
/// leilão e atesta vencedor e `lances_root`) e o intermediador (que atesta o depósito do escrow em R$).
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaAcceptVenda<'info> {
    pub industria: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, industria.key().as_ref()], bump = industria_part.bump)]
    pub industria_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_PARTICIPANTE, lote.cooperativa.as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    pub operador: Signer<'info>,
    pub intermediador: Signer<'info>,
    #[account(
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = operador @ EcolLoteError::NaoEhOperador,
        has_one = intermediador @ EcolLoteError::NaoEhIntermediador
    )]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_industria_accept_venda(
    ctx: Context<IndustriaAcceptVenda>,
    valor_centavos: u64,
    escrow_ref_hash: [u8; 32],
    lances_root: [u8; 32],
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    ctx.accounts.industria_part.exigir(Papel::Industria)?;
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    require!(
        !config.eh_chave_de_papel(&ctx.accounts.industria.key()),
        EcolLoteError::CarteiraReservada
    );
    require!(escrow_ref_hash != [0; 32], EcolLoteError::ReferenciaVazia);
    require!(lances_root != [0; 32], EcolLoteError::ReferenciaVazia);

    let agora = Clock::get()?.unix_timestamp;
    let prazo_entrega = agora
        .checked_add(config.prazo_entrega_s)
        .ok_or(EcolLoteError::Overflow)?;
    let tolerancia_peso_bps = config.tolerancia_peso_bps;
    let lote_key = ctx.accounts.lote.key();
    let industria = ctx.accounts.industria.key();
    let lote = &mut ctx.accounts.lote;
    require!(
        valor_centavos >= lote.preco_minimo_centavos,
        EcolLoteError::ValorAbaixoDoMinimo
    );
    let para = lote.estado.vender()?;
    lote.industria = industria;
    lote.valor_centavos = valor_centavos;
    lote.escrow_ref_hash = escrow_ref_hash;
    lote.lances_root = lances_root;
    lote.prazo_entrega = prazo_entrega;
    lote.tolerancia_peso_bps = tolerancia_peso_bps;
    let (de, seq) = lote.transicionar(para, agora)?;
    let seq_venda = lote.proximo_seq()?;

    emit_cpi!(LoteTransicionado {
        lote: lote_key,
        seq,
        de,
        para,
        ator: industria,
        ts: agora
    });
    emit_cpi!(LoteVendido {
        lote: lote_key,
        seq: seq_venda,
        industria,
        valor_centavos,
        escrow_ref_hash,
        lances_root,
    });
    Ok(())
}
