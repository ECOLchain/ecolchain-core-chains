use anchor_lang::prelude::*;
use ecol_lote::{program::EcolLote, EstadoLote, GlobalConfig, Lote, Material};
use solana_sha256_hasher::hashv;

use crate::{
    constants::{MAX_LOTES_POR_TX, SEED_AUTORIDADE, SEED_CONFIG, SEED_CREDITO},
    error::EcolCreditoError,
    events::{CreditoCriado, CreditoTransicionado, LoteAgregado},
    state::{Credito, CreditoConfig, EstadoCredito, PadraoRegistro},
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(credito_id: u64)]
pub struct OperadorCreateCredito<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = operador @ EcolCreditoError::NaoEhOperador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(
        init,
        payer = payer,
        space = 8 + Credito::INIT_SPACE,
        seeds = [SEED_CREDITO, &credito_id.to_le_bytes()],
        bump
    )]
    pub credito: Account<'info, Credito>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_create_credito(
    ctx: Context<OperadorCreateCredito>,
    credito_id: u64,
    padrao: PadraoRegistro,
    material: Material,
    vintage: u16,
    metodologia_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(
        metodologia_hash != [0; 32],
        EcolCreditoError::ReferenciaVazia
    );
    require!(vintage >= 2000, EcolCreditoError::ParametroInvalido);
    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let credito = &mut ctx.accounts.credito;
    credito.credito_id = credito_id;
    credito.padrao = padrao;
    credito.material = material;
    credito.vintage = vintage;
    credito.metodologia_hash = metodologia_hash;
    credito.estado = EstadoCredito::EmFormacao;
    credito.criado_em = agora;
    credito.atualizado_em = agora;
    credito.bump = ctx.bumps.credito;
    let seq = credito.proximo_seq()?;
    emit_cpi!(CreditoCriado {
        credito: credito_key,
        seq,
        credito_id,
        padrao,
        material,
        vintage,
        metodologia_hash,
    });
    Ok(())
}

/// Agrega lotes `Reciclado` ao crédito, marcando-os `Agregado` no `ecol_lote` por CPI.
/// Os lotes vão em `remaining_accounts` (graváveis). Um lote só pode ser agregado uma vez:
/// o próprio `ecol_lote` rejeita a transição fora de `Reciclado`, o que impede dupla contagem.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorAddLotes<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = operador @ EcolCreditoError::NaoEhOperador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(mut, seeds = [SEED_CREDITO, &credito.credito_id.to_le_bytes()], bump = credito.bump)]
    pub credito: Account<'info, Credito>,
    /// CHECK: PDA sem dados; só assina a CPI. Endereço fixado pelas seeds.
    #[account(seeds = [SEED_AUTORIDADE], bump)]
    pub autoridade: UncheckedAccount<'info>,
    /// Config do `ecol_lote` (owner e discriminator checados pelo `Account`).
    pub lote_config: Account<'info, GlobalConfig>,
    /// CHECK: event authority do `ecol_lote`; o próprio `ecol_lote` valida as seeds na CPI.
    pub lote_event_authority: UncheckedAccount<'info>,
    pub ecol_lote_program: Program<'info, EcolLote>,
}

fn ler_lote(conta: &AccountInfo) -> Result<Lote> {
    require_keys_eq!(
        *conta.owner,
        ecol_lote::ID,
        EcolCreditoError::LoteDeOutroPrograma
    );
    let dados = conta.try_borrow_data()?;
    Lote::try_deserialize(&mut &dados[..])
}

pub fn handle_operador_add_lotes<'info>(
    ctx: Context<'info, OperadorAddLotes<'info>>,
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    ctx.accounts.credito.estado.exigir_em_formacao()?;
    let lotes = ctx.remaining_accounts;
    require!(
        !lotes.is_empty() && lotes.len() <= MAX_LOTES_POR_TX,
        EcolCreditoError::QuantidadeDeLotesInvalida
    );

    let credito_key = ctx.accounts.credito.key();
    let material = ctx.accounts.credito.material;
    let seeds: &[&[u8]] = &[SEED_AUTORIDADE, &[ctx.bumps.autoridade]];

    for conta in lotes {
        // Material é imutável no lote: pode ser validado antes da CPI.
        let antes = ler_lote(conta)?;
        require!(
            antes.material == material,
            EcolCreditoError::MaterialDiferente
        );

        ecol_lote::cpi::credito_mark_agregado(
            CpiContext::new_with_signer(
                ecol_lote::ID,
                ecol_lote::cpi::accounts::CreditoMarkAgregado {
                    credito_autoridade: ctx.accounts.autoridade.to_account_info(),
                    config: ctx.accounts.lote_config.to_account_info(),
                    lote: conta.clone(),
                    event_authority: ctx.accounts.lote_event_authority.to_account_info(),
                    program: ctx.accounts.ecol_lote_program.to_account_info(),
                },
                &[seeds],
            ),
            credito_key,
        )?;

        // Estado relido depois da CPI: nada é decidido com dados anteriores a ela.
        let depois = ler_lote(conta)?;
        require!(
            depois.estado == EstadoLote::Agregado && depois.credito == credito_key,
            EcolCreditoError::AgregacaoNaoConfirmada
        );

        let credito = &mut ctx.accounts.credito;
        credito.lotes_hash = hashv(&[
            &credito.lotes_hash,
            conta.key.as_ref(),
            &depois.peso_g.to_le_bytes(),
        ])
        .to_bytes();
        credito.qtd_lotes = credito
            .qtd_lotes
            .checked_add(1)
            .ok_or(EcolCreditoError::Overflow)?;
        credito.peso_total_g = credito
            .peso_total_g
            .checked_add(depois.peso_g)
            .ok_or(EcolCreditoError::Overflow)?;
        let seq = credito.proximo_seq()?;
        let lotes_hash = credito.lotes_hash;
        emit_cpi!(LoteAgregado {
            credito: credito_key,
            seq,
            lote: conta.key(),
            peso_g: depois.peso_g,
            lotes_hash,
        });
    }
    Ok(())
}

/// Fecha a lista de lotes e submete o crédito ao validador.
#[event_cpi]
#[derive(Accounts)]
pub struct OperadorSubmitCredito<'info> {
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = operador @ EcolCreditoError::NaoEhOperador)]
    pub config: Account<'info, CreditoConfig>,
    #[account(mut, seeds = [SEED_CREDITO, &credito.credito_id.to_le_bytes()], bump = credito.bump)]
    pub credito: Account<'info, Credito>,
}

pub fn handle_operador_submit_credito(ctx: Context<OperadorSubmitCredito>) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    let agora = Clock::get()?.unix_timestamp;
    let credito_key = ctx.accounts.credito.key();
    let ator = ctx.accounts.operador.key();
    let credito = &mut ctx.accounts.credito;
    require!(credito.qtd_lotes > 0, EcolCreditoError::CreditoVazio);
    let para = credito.estado.submeter()?;
    let (de, seq) = credito.transicionar(para, agora)?;
    emit_cpi!(CreditoTransicionado {
        credito: credito_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    Ok(())
}
