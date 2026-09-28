use anchor_lang::prelude::*;
use ecol_credito::{Credito, EstadoCredito};
use ecol_lote::{Papel, Participante, SEED_PARTICIPANTE};

use crate::{
    constants::{MAX_FOLHAS, MAX_PROVA, SEED_CONFIG, SEED_DISTRIBUICAO},
    error::EcolReparticaoError,
    events::{DistribuicaoCriada, RepasseRegistrado, ResgateRegistrado},
    merkle,
    state::{Distribuicao, EstadoDistribuicao, ReparticaoConfig},
};

/// Publica os direitos de um crédito vendido. A receita vem do próprio `Credito`
/// (atestada pelo intermediador no `ecol_credito`); os tetos por papel são calculados aqui.
#[event_cpi]
#[derive(Accounts)]
#[instruction(bps_industria: u16, merkle_root: [u8; 32], num_folhas: u32)]
pub struct OperadorCreateDistribuicao<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub operador: Signer<'info>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = operador @ EcolReparticaoError::NaoEhOperador)]
    pub config: Account<'info, ReparticaoConfig>,
    /// Owner (`ecol_credito`) e discriminator checados pelo `Account`.
    pub credito: Account<'info, Credito>,
    #[account(
        init,
        payer = payer,
        space = Distribuicao::espaco(num_folhas),
        seeds = [SEED_DISTRIBUICAO, credito.key().as_ref()],
        bump
    )]
    pub distribuicao: Account<'info, Distribuicao>,
    pub system_program: Program<'info, System>,
}

pub fn handle_operador_create_distribuicao(
    ctx: Context<OperadorCreateDistribuicao>,
    bps_industria: u16,
    merkle_root: [u8; 32],
    num_folhas: u32,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_ativo()?;
    let credito = &ctx.accounts.credito;
    require!(
        matches!(
            credito.estado,
            EstadoCredito::Vendido | EstadoCredito::Aposentado
        ) && credito.valor_venda_centavos > 0,
        EcolReparticaoError::CreditoNaoVendido
    );
    require!(
        num_folhas > 0 && num_folhas <= MAX_FOLHAS,
        EcolReparticaoError::NumFolhasInvalido
    );
    require!(merkle_root != [0; 32], EcolReparticaoError::ReferenciaVazia);

    let bps = config.bps;
    let receita = credito.valor_venda_centavos;
    let credito_key = credito.key();
    let dist_key = ctx.accounts.distribuicao.key();
    let d = &mut ctx.accounts.distribuicao;
    d.calcular(receita, bps, bps_industria)?;
    d.credito = credito_key;
    d.merkle_root = merkle_root;
    d.num_folhas = num_folhas;
    d.resgatados = vec![0; Distribuicao::bytes_bitmap(num_folhas)];
    d.estado = EstadoDistribuicao::Pendente;
    d.criado_em = Clock::get()?.unix_timestamp;
    d.bump = ctx.bumps.distribuicao;
    let seq = d.proximo_seq()?;
    let (total_por_papel, ecolchain_centavos) = (d.total_por_papel, d.ecolchain_centavos);
    emit_cpi!(DistribuicaoCriada {
        distribuicao: dist_key,
        seq,
        credito: credito_key,
        receita_centavos: receita,
        bps_industria,
        total_por_papel,
        ecolchain_centavos,
        merkle_root,
        num_folhas,
    });
    Ok(())
}

/// O intermediador atesta que repassou a parte dos cupons à Zupy e a parte da EcolChain.
#[event_cpi]
#[derive(Accounts)]
pub struct IntermediadorRecordRepasse<'info> {
    pub intermediador: Signer<'info>,
    #[account(
        seeds = [SEED_CONFIG],
        bump = config.bump,
        has_one = intermediador @ EcolReparticaoError::NaoEhIntermediador
    )]
    pub config: Account<'info, ReparticaoConfig>,
    #[account(mut, seeds = [SEED_DISTRIBUICAO, distribuicao.credito.as_ref()], bump = distribuicao.bump)]
    pub distribuicao: Account<'info, Distribuicao>,
}

pub fn handle_intermediador_record_repasse(
    ctx: Context<IntermediadorRecordRepasse>,
    zupy_ref_hash: [u8; 32],
    ecolchain_ref_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(
        zupy_ref_hash != [0; 32] && ecolchain_ref_hash != [0; 32],
        EcolReparticaoError::ReferenciaVazia
    );
    let dist_key = ctx.accounts.distribuicao.key();
    let d = &mut ctx.accounts.distribuicao;
    require!(
        d.estado == EstadoDistribuicao::Pendente,
        EcolReparticaoError::EstadoInvalido
    );
    d.estado = EstadoDistribuicao::Financiada;
    d.repasse_zupy_ref_hash = zupy_ref_hash;
    d.repasse_ecolchain_ref_hash = ecolchain_ref_hash;
    let seq = d.proximo_seq()?;
    let cupons_centavos = d.total_cupons()?;
    let ecolchain_centavos = d.ecolchain_centavos;
    emit_cpi!(RepasseRegistrado {
        distribuicao: dist_key,
        seq,
        cupons_centavos,
        ecolchain_centavos,
        zupy_ref_hash,
        ecolchain_ref_hash,
    });
    Ok(())
}

/// Resgate de um direito em cupons: o beneficiário (carteira embutida) e a Zupy assinam juntos.
/// A folha é provada contra a raiz publicada; cada folha resgata uma vez; cada papel respeita seu teto.
#[event_cpi]
#[derive(Accounts)]
pub struct ZupyRecordResgate<'info> {
    pub zupy: Signer<'info>,
    pub beneficiario: Signer<'info>,
    /// Cadastro do beneficiário no `ecol_lote` (owner, discriminator e seeds checados).
    #[account(
        seeds = [SEED_PARTICIPANTE, beneficiario.key().as_ref()],
        bump = beneficiario_part.bump,
        seeds::program = ecol_lote::ID
    )]
    pub beneficiario_part: Account<'info, Participante>,
    #[account(seeds = [SEED_CONFIG], bump = config.bump, has_one = zupy @ EcolReparticaoError::NaoEhZupy)]
    pub config: Account<'info, ReparticaoConfig>,
    #[account(mut, seeds = [SEED_DISTRIBUICAO, distribuicao.credito.as_ref()], bump = distribuicao.bump)]
    pub distribuicao: Account<'info, Distribuicao>,
}

pub fn handle_zupy_record_resgate(
    ctx: Context<ZupyRecordResgate>,
    indice: u32,
    papel: Papel,
    valor_centavos: u64,
    prova: Vec<[u8; 32]>,
    recibo_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_ativo()?;
    require!(recibo_hash != [0; 32], EcolReparticaoError::ReferenciaVazia);
    require!(prova.len() <= MAX_PROVA, EcolReparticaoError::ProvaInvalida);
    // Direito já adquirido não depende de o participante continuar ativo; o papel precisa bater.
    require!(
        ctx.accounts.beneficiario_part.papel == papel,
        EcolReparticaoError::PapelIncorreto
    );
    let beneficiario = ctx.accounts.beneficiario.key();
    let dist_key = ctx.accounts.distribuicao.key();
    let d = &mut ctx.accounts.distribuicao;
    require!(
        d.estado == EstadoDistribuicao::Financiada,
        EcolReparticaoError::EstadoInvalido
    );
    let folha = merkle::folha(indice, &beneficiario, papel, valor_centavos);
    require!(
        merkle::verificar(&prova, folha, &d.merkle_root),
        EcolReparticaoError::ProvaInvalida
    );
    d.registrar_resgate(indice, papel, valor_centavos)?;
    let seq = d.proximo_seq()?;
    emit_cpi!(ResgateRegistrado {
        distribuicao: dist_key,
        seq,
        indice,
        beneficiario,
        papel,
        valor_centavos,
        recibo_hash,
    });
    Ok(())
}
