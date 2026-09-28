use anchor_lang::prelude::*;

use crate::{
    ativo::{self, ContasAsset},
    constants::{
        BPS_BASE, MAX_ENTREGAS_POR_TX, SEED_ASSET, SEED_AUTORIDADE_ASSET, SEED_BALANCA,
        SEED_GLOBAL, SEED_LOTE, SEED_MATERIAL, SEED_PARTICIPANTE, URI_MAX_LEN,
    },
    error::EcolLoteError,
    events::{
        EntregaDevolvida, LoteCriado, LoteMontagemIniciada, LoteTransicionado, PesagemAtestada,
    },
    instructions::entregas::ler_entrega,
    pesagem::{self, TipoPesagem},
    state::{Balanca, EstadoLote, GlobalConfig, Lote, Material, Papel, Participante},
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosLote {
    pub lote_id: u64,
    /// Código do material cadastrado; a conta `material` correspondente deve estar ativa.
    pub material: u16,
    pub evidencias_hash: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosFechamento {
    /// Peso do lote consolidado em gramas, exatamente como assinado pela balança.
    pub peso_g: u64,
    /// Timestamp assinado pela balança.
    pub ts_pesagem: i64,
    /// Metadados off-chain do Core asset (JSON em Arweave/IPFS).
    pub uri: String,
}

/// Abre um lote de venda para montagem. Os lotes de origem entram com `cooperativa_add_entregas`
/// e a pesagem do consolidado vem no fechamento (`cooperativa_fechar_lote`).
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
    #[account(seeds = [SEED_MATERIAL, &dados.material.to_le_bytes()], bump = material_cadastro.bump)]
    pub material_cadastro: Box<Account<'info, Material>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Lote::INIT_SPACE,
        seeds = [SEED_LOTE, cooperativa.key().as_ref(), &dados.lote_id.to_le_bytes()],
        bump
    )]
    pub lote: Box<Account<'info, Lote>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cooperativa_create_lote(
    ctx: Context<CooperativaCreateLote>,
    dados: DadosLote,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    ctx.accounts.material_cadastro.exigir_ativo()?;
    require!(
        dados.evidencias_hash != [0; 32],
        EcolLoteError::ReferenciaVazia
    );

    let agora = Clock::get()?.unix_timestamp;
    let cooperativa = ctx.accounts.cooperativa.key();
    let lote_key = ctx.accounts.lote.key();
    let lote = &mut ctx.accounts.lote;
    lote.cooperativa = cooperativa;
    lote.lote_id = dados.lote_id;
    lote.material = dados.material;
    lote.evidencias_hash = dados.evidencias_hash;
    lote.estado = EstadoLote::EmMontagem;
    lote.criado_em = agora;
    lote.atualizado_em = agora;
    lote.bump = ctx.bumps.lote;
    let seq = lote.proximo_seq()?;

    emit_cpi!(LoteMontagemIniciada {
        lote: lote_key,
        seq,
        cooperativa,
        lote_id: dados.lote_id,
        material: dados.material,
        evidencias_hash: dados.evidencias_hash,
        ts: agora,
    });
    Ok(())
}

/// Fecha o lote de venda com a pesagem do consolidado, assinada pela balança da cooperativa, e cria
/// o Core asset. O peso tem de ficar na faixa aceita sobre a soma dos lotes de origem.
/// A transação deve ter, imediatamente antes, a instrução Ed25519 com a assinatura da balança.
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaFecharLote<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()], bump = balanca.bump)]
    pub balanca: Box<Account<'info, Balanca>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = cooperativa @ EcolLoteError::NaoEhCooperativaDoLote
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

pub fn handle_cooperativa_fechar_lote(
    ctx: Context<CooperativaFecharLote>,
    dados: DadosFechamento,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    let cooperativa = ctx.accounts.cooperativa.key();
    ctx.accounts.balanca.exigir_ativa_de(&cooperativa)?;
    require!(dados.peso_g > 0, EcolLoteError::PesoInvalido);
    require!(
        !dados.uri.is_empty() && dados.uri.len() <= URI_MAX_LEN,
        EcolLoteError::UriInvalida
    );
    require!(
        ctx.accounts.lote.qtd_entregas > 0,
        EcolLoteError::LoteSemEntregas
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
    // Pesagem autêntica primeiro; depois, se o consolidado é compatível com os lotes de origem.
    let (perda_bps, excesso_bps) = ctx.accounts.config.tolerancia_montagem();
    exigir_peso_na_faixa(
        dados.peso_g,
        ctx.accounts.lote.peso_entregas_g,
        perda_bps,
        excesso_bps,
    )?;
    // Antes do CPI: só um lote em montagem vira `Criado`.
    let para = ctx.accounts.lote.estado.fechar()?;

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
        ctx.accounts.lote.lote_id,
        dados.uri,
        ctx.bumps.asset,
    )?;

    let balanca_key = ctx.accounts.balanca.key();
    let asset_key = ctx.accounts.asset.key();
    let lote = &mut ctx.accounts.lote;
    lote.peso_g = dados.peso_g;
    lote.balanca_origem = balanca_key;
    lote.ts_pesagem = dados.ts_pesagem;
    lote.asset = asset_key;
    lote.asset_queimavel = true;
    let (_, seq) = lote.transicionar(para, agora)?;
    let seq_pesagem = lote.proximo_seq()?;

    emit_cpi!(LoteCriado {
        lote: lote_key,
        seq,
        cooperativa,
        lote_id: lote.lote_id,
        material: lote.material,
        peso_g: dados.peso_g,
        evidencias_hash: lote.evidencias_hash,
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

/// `soma × (1 − perda) ≤ peso ≤ soma × (1 + excesso)`, em inteiros.
pub fn exigir_peso_na_faixa(
    peso_g: u64,
    soma_g: u64,
    perda_bps: u16,
    excesso_bps: u16,
) -> Result<()> {
    let base = u128::from(BPS_BASE);
    let peso = u128::from(peso_g)
        .checked_mul(base)
        .ok_or(EcolLoteError::Overflow)?;
    let minimo = u128::from(soma_g)
        .checked_mul(base.saturating_sub(u128::from(perda_bps)))
        .ok_or(EcolLoteError::Overflow)?;
    let maximo = u128::from(soma_g)
        .checked_mul(
            base.checked_add(u128::from(excesso_bps))
                .ok_or(EcolLoteError::Overflow)?,
        )
        .ok_or(EcolLoteError::Overflow)?;
    require!(
        peso >= minimo && peso <= maximo,
        EcolLoteError::PesoConsolidadoForaDaFaixa
    );
    Ok(())
}

/// Desfaz a união antes da venda: devolve os lotes de origem (em `remaining_accounts`, graváveis,
/// até 10 por transação). O lote fica `EmDesmontagem` até o último sair e então vira `Desfeito`,
/// com o Core asset queimado (lotes fechados desde a ADR 0010).
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaDesmontarLote<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = cooperativa @ EcolLoteError::NaoEhCooperativaDoLote
    )]
    pub lote: Box<Account<'info, Lote>>,
    /// CHECK: asset do lote (PDA); só é tocado se o lote tiver asset queimável.
    #[account(mut, seeds = [SEED_ASSET, lote.key().as_ref()], bump)]
    pub asset: UncheckedAccount<'info>,
    /// CHECK: PDA sem dados, só assina como autoridade do asset; endereço fixado pelas seeds.
    #[account(seeds = [SEED_AUTORIDADE_ASSET], bump)]
    pub autoridade_asset: UncheckedAccount<'info>,
    /// CHECK: endereço fixo do programa Metaplex Core.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cooperativa_desmontar_lote<'info>(
    ctx: Context<'info, CooperativaDesmontarLote<'info>>,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let n = ctx.remaining_accounts.len();
    require!(
        n <= MAX_ENTREGAS_POR_TX,
        EcolLoteError::QuantidadeDeEntregasInvalida
    );
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();

    // Primeira chamada: entra em desmontagem (a transição valida o estado).
    if ctx.accounts.lote.estado != EstadoLote::EmDesmontagem {
        let lote = &mut ctx.accounts.lote;
        let para = lote.estado.iniciar_desmontagem()?;
        let (de, seq) = lote.transicionar(para, agora)?;
        emit_cpi!(LoteTransicionado {
            lote: lote_key,
            seq,
            de,
            para,
            ator,
            ts: agora,
        });
    }
    // Sem origens na transação só faz sentido para concluir um lote já vazio.
    require!(
        n > 0 || ctx.accounts.lote.qtd_entregas == 0,
        EcolLoteError::QuantidadeDeEntregasInvalida
    );

    for conta in ctx.remaining_accounts.iter() {
        let mut entrega = ler_entrega(conta)?;
        require_keys_eq!(entrega.lote, lote_key, EcolLoteError::EntregaDeOutroLote);
        let lote = &mut ctx.accounts.lote;
        lote.qtd_entregas = lote
            .qtd_entregas
            .checked_sub(1)
            .ok_or(EcolLoteError::Overflow)?;
        lote.peso_entregas_g = lote
            .peso_entregas_g
            .checked_sub(entrega.peso_g)
            .ok_or(EcolLoteError::Overflow)?;
        if !entrega.com_coletor() {
            lote.peso_sem_coletor_g = lote
                .peso_sem_coletor_g
                .checked_sub(entrega.peso_g)
                .ok_or(EcolLoteError::Overflow)?;
        }
        entrega.lote = Pubkey::default();
        let seq = lote.proximo_seq()?;
        emit_cpi!(EntregaDevolvida {
            lote: lote_key,
            seq,
            entrega: conta.key(),
            peso_g: entrega.peso_g,
        });
        entrega.exit(&crate::ID)?;
    }

    if ctx.accounts.lote.qtd_entregas == 0 {
        if ctx.accounts.lote.asset_queimavel {
            let contas = ContasAsset {
                mpl_core_program: &ctx.accounts.mpl_core_program,
                asset: &ctx.accounts.asset,
                autoridade_asset: &ctx.accounts.autoridade_asset,
                payer: &ctx.accounts.payer,
                system_program: &ctx.accounts.system_program,
            };
            ativo::queimar(&contas, ctx.bumps.autoridade_asset)?;
        }
        let lote = &mut ctx.accounts.lote;
        let para = lote.estado.concluir_desmontagem()?;
        let (de, seq) = lote.transicionar(para, agora)?;
        emit_cpi!(LoteTransicionado {
            lote: lote_key,
            seq,
            de,
            para,
            ator,
            ts: agora,
        });
    }
    Ok(())
}
