//! Lances on-chain no leilão (ADR 0013). A indústria dá o lance sozinha; o maior lance da rodada fica
//! no lote (`lance_lider`, `maior_lance_centavos`) e cada indústria tem a sua conta `Lance`. Depois do
//! prazo, a venda (três assinaturas) só pode ir ao líder, pelo valor do lance.
use anchor_lang::prelude::*;

use crate::{
    constants::{SEED_CARTEIRA, SEED_GLOBAL, SEED_LANCE, SEED_LOTE, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::LanceDado,
    state::{exigir_assinatura, Carteira, GlobalConfig, Lance, Lote, Papel, Participante},
};

/// Primeiro lance da indústria no lote (cria a conta `Lance`).
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaDarLance<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: carteira titular da indústria; conferida pelo cadastro e pela assinatura.
    pub industria: UncheckedAccount<'info>,
    pub industria_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, industria_assinante.key().as_ref()],
        bump = industria_carteira.bump
    )]
    pub industria_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, industria.key().as_ref()], bump = industria_part.bump)]
    pub industria_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Box<Account<'info, Lote>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Lance::INIT_SPACE,
        seeds = [SEED_LANCE, lote.key().as_ref(), industria.key().as_ref()],
        bump
    )]
    pub lance: Box<Account<'info, Lance>>,
    pub system_program: Program<'info, System>,
}

/// Aumenta o lance (ou volta a dar lance numa nova rodada) com a conta `Lance` já existente.
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaAumentarLance<'info> {
    /// CHECK: carteira titular da indústria; conferida pelo cadastro e pela assinatura.
    pub industria: UncheckedAccount<'info>,
    pub industria_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, industria_assinante.key().as_ref()],
        bump = industria_carteira.bump
    )]
    pub industria_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, industria.key().as_ref()], bump = industria_part.bump)]
    pub industria_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Box<Account<'info, Lote>>,
    #[account(
        mut,
        seeds = [SEED_LANCE, lote.key().as_ref(), industria.key().as_ref()],
        bump = lance.bump
    )]
    pub lance: Box<Account<'info, Lance>>,
}

/// Assinatura, papel e chave: as mesmas regras nas duas instruções.
fn exigir_industria(
    config: &GlobalConfig,
    industria: &Pubkey,
    assinante: &Pubkey,
    carteira: Option<&Carteira>,
    part: &Participante,
) -> Result<()> {
    config.exigir_modo_normal()?;
    exigir_assinatura(industria, assinante, carteira)?;
    part.exigir(Papel::Industria)?;
    require!(
        !config.eh_chave_de_papel(industria),
        EcolLoteError::CarteiraReservada
    );
    Ok(())
}

/// Grava o lance na conta da indústria, aplica no lote e emite o evento.
fn registrar(
    lote_key: Pubkey,
    lote: &mut Lote,
    lance: &mut Lance,
    industria: Pubkey,
    valor_centavos: u64,
) -> Result<LanceDado> {
    let agora = Clock::get()?.unix_timestamp;
    let prazo_leilao = lote.aplicar_lance(industria, valor_centavos, agora)?;
    lote.atualizado_em = agora;
    let seq = lote.proximo_seq()?;
    lance.valor_centavos = valor_centavos;
    lance.prazo_leilao = prazo_leilao;
    lance.atualizado_em = agora;
    Ok(LanceDado {
        lote: lote_key,
        seq,
        industria,
        valor_centavos,
        prazo_leilao,
        ts: agora,
    })
}

pub fn handle_industria_dar_lance(
    ctx: Context<IndustriaDarLance>,
    valor_centavos: u64,
) -> Result<()> {
    let a = &ctx.accounts;
    let industria = a.industria.key();
    exigir_industria(
        &a.config,
        &industria,
        &a.industria_assinante.key(),
        a.industria_carteira.as_deref().map(|c| &**c),
        &a.industria_part,
    )?;
    let lote_key = ctx.accounts.lote.key();
    let pagador = ctx.accounts.payer.key();
    let bump = ctx.bumps.lance;
    let lance = &mut ctx.accounts.lance;
    lance.lote = lote_key;
    lance.industria = industria;
    lance.pagador = pagador;
    lance.criado_em = Clock::get()?.unix_timestamp;
    lance.bump = bump;
    let evento = registrar(
        lote_key,
        &mut ctx.accounts.lote,
        lance,
        industria,
        valor_centavos,
    )?;
    emit_cpi!(evento);
    Ok(())
}

pub fn handle_industria_aumentar_lance(
    ctx: Context<IndustriaAumentarLance>,
    valor_centavos: u64,
) -> Result<()> {
    let a = &ctx.accounts;
    let industria = a.industria.key();
    exigir_industria(
        &a.config,
        &industria,
        &a.industria_assinante.key(),
        a.industria_carteira.as_deref().map(|c| &**c),
        &a.industria_part,
    )?;
    let lote_key = ctx.accounts.lote.key();
    let evento = registrar(
        lote_key,
        &mut ctx.accounts.lote,
        &mut ctx.accounts.lance,
        industria,
        valor_centavos,
    )?;
    emit_cpi!(evento);
    Ok(())
}

/// Crank público: fecha o lance de uma rodada que acabou (lote vendido, retirado do leilão ou
/// reanunciado) e devolve o rent a quem pagou.
#[derive(Accounts)]
pub struct PublicCloseLance<'info> {
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Box<Account<'info, Lote>>,
    #[account(
        mut,
        close = pagador,
        seeds = [SEED_LANCE, lote.key().as_ref(), lance.industria.as_ref()],
        bump = lance.bump,
        has_one = lote @ EcolLoteError::ParametroInvalido,
        has_one = pagador @ EcolLoteError::ParametroInvalido
    )]
    pub lance: Box<Account<'info, Lance>>,
    /// CHECK: recebe o rent; conferido por `has_one = pagador`.
    #[account(mut)]
    pub pagador: UncheckedAccount<'info>,
}

pub fn handle_public_close_lance(ctx: Context<PublicCloseLance>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let lote = &ctx.accounts.lote;
    // Enquanto o lote está na mesma rodada (anunciado com o mesmo prazo), o lance vale.
    let em_vigor = !lote.venda_direta
        && matches!(lote.estado, crate::state::EstadoLote::Anunciado { prazo_leilao } if prazo_leilao == ctx.accounts.lance.prazo_leilao);
    require!(!em_vigor, EcolLoteError::LanceEmVigor);
    Ok(())
}
