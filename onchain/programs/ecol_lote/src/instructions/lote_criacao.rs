use anchor_lang::prelude::*;

use crate::{
    ativo::{self, ContasAsset},
    constants::{
        SEED_ASSET, SEED_AUTORIDADE_ASSET, SEED_BALANCA, SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE,
        URI_MAX_LEN,
    },
    error::EcolLoteError,
    events::{LoteCriado, PesagemAtestada},
    pesagem::{self, TipoPesagem},
    state::{Balanca, EstadoLote, GlobalConfig, Lote, Material, Papel, Participante},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosLote {
    pub lote_id: u64,
    pub material: Material,
    /// Peso em gramas, exatamente como assinado pela balança.
    pub peso_g: u64,
    /// Timestamp assinado pela balança.
    pub ts_pesagem: i64,
    pub coletor_ref: [u8; 32],
    pub evidencias_hash: [u8; 32],
    /// Metadados off-chain do Core asset (JSON em Arweave/IPFS).
    pub uri: String,
}

/// Cria o lote a partir de uma pesagem assinada pela balança da cooperativa.
/// A transação deve ter, imediatamente antes, a instrução Ed25519 com a assinatura da balança.
#[event_cpi]
#[derive(Accounts)]
#[instruction(dados: DadosLote)]
pub struct CooperativaCreateLote<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()],
        bump = balanca.bump
    )]
    pub balanca: Box<Account<'info, Balanca>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Lote::INIT_SPACE,
        seeds = [SEED_LOTE, cooperativa.key().as_ref(), &dados.lote_id.to_le_bytes()],
        bump
    )]
    pub lote: Box<Account<'info, Lote>>,
    /// CHECK: conta nova, criada pelo Metaplex Core nesta instrução; endereço fixado pelas seeds.
    #[account(mut, seeds = [SEED_ASSET, lote.key().as_ref()], bump)]
    pub asset: UncheckedAccount<'info>,
    /// CHECK: PDA sem dados, só assina como autoridade do asset; endereço fixado pelas seeds.
    #[account(seeds = [SEED_AUTORIDADE_ASSET], bump)]
    pub autoridade_asset: UncheckedAccount<'info>,
    /// CHECK: endereço fixo do programa Metaplex Core.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    /// CHECK: endereço fixo do sysvar de instruções.
    #[account(address = solana_instructions_sysvar::ID)]
    pub instrucoes_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cooperativa_create_lote(
    ctx: Context<CooperativaCreateLote>,
    dados: DadosLote,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    let cooperativa = ctx.accounts.cooperativa.key();
    ctx.accounts.balanca.exigir_ativa_de(&cooperativa)?;
    require!(dados.peso_g > 0, EcolLoteError::PesoInvalido);
    require!(dados.coletor_ref != [0; 32], EcolLoteError::ReferenciaVazia);
    require!(
        dados.evidencias_hash != [0; 32],
        EcolLoteError::ReferenciaVazia
    );
    require!(
        !dados.uri.is_empty() && dados.uri.len() <= URI_MAX_LEN,
        EcolLoteError::UriInvalida
    );

    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    pesagem::exigir_pesagem_recente(dados.ts_pesagem, agora)?;
    pesagem::verificar_assinatura_balanca(
        &ctx.accounts.instrucoes_sysvar,
        &ctx.accounts.balanca.dispositivo,
        &pesagem::mensagem(
            TipoPesagem::Origem,
            &lote_key,
            dados.peso_g,
            dados.ts_pesagem,
        ),
    )?;

    let contas = ContasAsset {
        mpl_core_program: &ctx.accounts.mpl_core_program,
        asset: &ctx.accounts.asset,
        autoridade_asset: &ctx.accounts.autoridade_asset,
        payer: &ctx.accounts.payer,
        system_program: &ctx.accounts.system_program,
    };
    ativo::criar(
        &contas,
        &ctx.accounts.cooperativa,
        &lote_key,
        dados.lote_id,
        dados.uri,
        ctx.bumps.asset,
    )?;

    let balanca_key = ctx.accounts.balanca.key();
    let asset_key = ctx.accounts.asset.key();
    let lote = &mut ctx.accounts.lote;
    lote.cooperativa = cooperativa;
    lote.lote_id = dados.lote_id;
    lote.material = dados.material;
    lote.peso_g = dados.peso_g;
    lote.coletor_ref = dados.coletor_ref;
    lote.evidencias_hash = dados.evidencias_hash;
    lote.balanca_origem = balanca_key;
    lote.ts_pesagem = dados.ts_pesagem;
    lote.asset = asset_key;
    lote.estado = EstadoLote::Criado;
    lote.criado_em = agora;
    lote.atualizado_em = agora;
    lote.bump = ctx.bumps.lote;
    let seq = lote.proximo_seq()?;
    let seq_pesagem = lote.proximo_seq()?;

    emit_cpi!(LoteCriado {
        lote: lote_key,
        seq,
        cooperativa,
        lote_id: dados.lote_id,
        material: dados.material,
        peso_g: dados.peso_g,
        coletor_ref: dados.coletor_ref,
        evidencias_hash: dados.evidencias_hash,
        ts: agora,
    });
    emit_cpi!(PesagemAtestada {
        lote: lote_key,
        seq: seq_pesagem,
        balanca: balanca_key,
        peso_g: dados.peso_g,
        ts_pesagem: dados.ts_pesagem,
        recebimento: false,
    });
    Ok(())
}
