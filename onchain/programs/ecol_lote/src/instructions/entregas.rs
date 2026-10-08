use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;

use crate::{
    constants::{
        MAX_ENTREGAS_POR_TX, SEED_BALANCA, SEED_CARTEIRA, SEED_COLETA, SEED_ENTREGA, SEED_GLOBAL,
        SEED_LOTE, SEED_MATERIAL, SEED_PARTICIPANTE, SEED_VARIACAO,
    },
    error::EcolLoteError,
    events::{ColetaAtualizada, EntregaNoLote, EntregaRegistrada},
    pesagem::{self, TipoPesagem},
    state::{
        exigir_assinatura, Balanca, Carteira, Coleta, Entrega, EstadoColeta, EstadoLote,
        GlobalConfig, Lote, Material, MaterialVariacao, OrigemEntrega, Participante,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosEntrega {
    pub entrega_id: u64,
    pub origem: OrigemEntrega,
    /// Hash da referência da origem (coletor, nota, recibo, ticket); nunca o texto.
    pub origem_ref: [u8; 32],
    pub material: u16,
    /// Peso em gramas, exatamente como assinado pela balança.
    pub peso_g: u64,
    /// Timestamp assinado pela balança.
    pub ts_pesagem: i64,
    /// Índice da variação do material (0 se o material não tem variações).
    pub variacao: u8,
    /// Garrafas aproximadas (só em material que conta garrafas; 0 nos demais).
    pub qtd_garrafas: u32,
}

/// Registra um lote de origem (com ou sem coletor), pesado pela balança da cooperativa ou Clean Tech.
/// A transação deve ter, imediatamente antes, a instrução Ed25519 da balança sobre
/// `(domínio, Entrega, entrega, peso, ts)`. Com origem `Importador`, recebe a coleta enviada a este
/// destino e a marca como entregue.
#[event_cpi]
#[derive(Accounts)]
#[instruction(dados: DadosEntrega)]
pub struct CooperativaRegisterEntrega<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: carteira titular da cooperativa; conferida pelo cadastro e pela assinatura.
    pub cooperativa: UncheckedAccount<'info>,
    /// Titular ou carteira vinculada a ele.
    pub cooperativa_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, cooperativa_assinante.key().as_ref()],
        bump = cooperativa_carteira.bump
    )]
    pub cooperativa_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()], bump = balanca.bump)]
    pub balanca: Box<Account<'info, Balanca>>,
    #[account(seeds = [SEED_MATERIAL, &dados.material.to_le_bytes()], bump = material_cadastro.bump)]
    pub material_cadastro: Box<Account<'info, Material>>,
    #[account(
        seeds = [SEED_VARIACAO, &dados.material.to_le_bytes(), &[dados.variacao]],
        bump = variacao_cadastro.bump
    )]
    pub variacao_cadastro: Option<Box<Account<'info, MaterialVariacao>>>,
    #[account(
        mut,
        seeds = [SEED_COLETA, coleta.importador.as_ref(), &coleta.coleta_id.to_le_bytes()],
        bump = coleta.bump
    )]
    pub coleta: Option<Box<Account<'info, Coleta>>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Entrega::INIT_SPACE,
        seeds = [SEED_ENTREGA, cooperativa.key().as_ref(), &dados.entrega_id.to_le_bytes()],
        bump
    )]
    pub entrega: Box<Account<'info, Entrega>>,
    /// CHECK: endereço fixo do sysvar de instruções.
    #[account(address = solana_instructions_sysvar::ID)]
    pub instrucoes_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_cooperativa_register_entrega(
    ctx: Context<CooperativaRegisterEntrega>,
    dados: DadosEntrega,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let cooperativa = ctx.accounts.cooperativa.key();
    exigir_assinatura(
        &cooperativa,
        &ctx.accounts.cooperativa_assinante.key(),
        ctx.accounts.cooperativa_carteira.as_deref().map(|c| &**c),
    )?;
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
    ctx.accounts.balanca.exigir_ativa_de(&cooperativa)?;
    let material = &ctx.accounts.material_cadastro;
    material.exigir_ativo()?;
    let variacao =
        material.variacao_do_lote(ctx.accounts.variacao_cadastro.as_deref().map(|v| &**v))?;
    require!(variacao == dados.variacao, EcolLoteError::VariacaoInvalida);
    material.exigir_garrafas(dados.qtd_garrafas)?;
    require!(dados.peso_g > 0, EcolLoteError::PesoInvalido);
    // Toda origem tem referência: coletor, documento da doação/compra, ticket da triagem ou coleta.
    require!(dados.origem_ref != [0; 32], EcolLoteError::ReferenciaVazia);

    let agora = Clock::get()?.unix_timestamp;
    let entrega_key = ctx.accounts.entrega.key();
    pesagem::exigir_pesagem_recente(dados.ts_pesagem, agora)?;
    pesagem::verificar_assinatura_balanca(
        &ctx.accounts.instrucoes_sysvar,
        &ctx.accounts.balanca.dispositivo,
        &pesagem::mensagem(
            TipoPesagem::Entrega,
            &entrega_key,
            dados.peso_g,
            dados.ts_pesagem,
        ),
    )?;

    // Coleta do importador: a referência é a própria conta, enviada a este destino.
    match (dados.origem, ctx.accounts.coleta.as_deref_mut()) {
        (OrigemEntrega::Importador, Some(coleta)) => {
            require_keys_eq!(
                Pubkey::new_from_array(dados.origem_ref),
                coleta.key(),
                EcolLoteError::ColetaAusente
            );
            coleta.exigir_estado(EstadoColeta::EmEntrega)?;
            require_keys_eq!(
                coleta.destino,
                cooperativa,
                EcolLoteError::ColetaDeOutroDestino
            );
            require!(
                coleta.material == dados.material && coleta.variacao == dados.variacao,
                EcolLoteError::ColetaDivergente
            );
            coleta.estado = EstadoColeta::Entregue;
            coleta.entrega = entrega_key;
            coleta.atualizado_em = agora;
            emit_cpi!(ColetaAtualizada {
                coleta: coleta.key(),
                importador: coleta.importador,
                de: Some(EstadoColeta::EmEntrega),
                para: EstadoColeta::Entregue,
                destino: cooperativa,
                entrega: entrega_key,
                nf_reciclagem_hash: [0; 32],
                ator: ctx.accounts.cooperativa_assinante.key(),
                ts: agora,
            });
        }
        (OrigemEntrega::Importador, None) | (_, Some(_)) => {
            return err!(EcolLoteError::ColetaAusente);
        }
        (_, None) => {}
    }

    let balanca_key = ctx.accounts.balanca.key();
    let entrega = &mut ctx.accounts.entrega;
    entrega.cooperativa = cooperativa;
    entrega.entrega_id = dados.entrega_id;
    entrega.origem = dados.origem;
    entrega.origem_ref = dados.origem_ref;
    entrega.material = dados.material;
    entrega.peso_g = dados.peso_g;
    entrega.balanca = balanca_key;
    entrega.ts_pesagem = dados.ts_pesagem;
    entrega.criado_em = agora;
    entrega.bump = ctx.bumps.entrega;
    entrega.variacao = dados.variacao;
    entrega.qtd_garrafas = dados.qtd_garrafas;

    emit_cpi!(EntregaRegistrada {
        entrega: entrega_key,
        cooperativa,
        entrega_id: dados.entrega_id,
        origem: dados.origem,
        origem_ref: dados.origem_ref,
        material: dados.material,
        peso_g: dados.peso_g,
        balanca: balanca_key,
        ts_pesagem: dados.ts_pesagem,
        variacao: dados.variacao,
        qtd_garrafas: dados.qtd_garrafas,
    });
    Ok(())
}

/// Monta o lote de venda: vincula lotes de origem disponíveis (em `remaining_accounts`, graváveis)
/// a um lote ainda `EmMontagem`. Mesma cooperativa, mesmo material; cada origem entra em um só lote.
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaAddEntregas<'info> {
    /// CHECK: carteira titular da cooperativa; conferida contra o lote e pela assinatura.
    pub cooperativa: UncheckedAccount<'info>,
    pub cooperativa_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, cooperativa_assinante.key().as_ref()],
        bump = cooperativa_carteira.bump
    )]
    pub cooperativa_carteira: Option<Account<'info, Carteira>>,
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

pub fn handle_cooperativa_add_entregas<'info>(
    ctx: Context<'info, CooperativaAddEntregas<'info>>,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    exigir_assinatura(
        &ctx.accounts.cooperativa.key(),
        &ctx.accounts.cooperativa_assinante.key(),
        ctx.accounts.cooperativa_carteira.as_deref(),
    )?;
    let n = ctx.remaining_accounts.len();
    require!(
        n > 0 && n <= MAX_ENTREGAS_POR_TX,
        EcolLoteError::QuantidadeDeEntregasInvalida
    );
    let lote_key = ctx.accounts.lote.key();
    let lote = &mut ctx.accounts.lote;
    require!(
        lote.estado == EstadoLote::EmMontagem,
        EcolLoteError::TransicaoInvalida
    );

    for conta in ctx.remaining_accounts.iter() {
        let mut entrega = ler_entrega(conta)?;
        require_keys_eq!(
            entrega.cooperativa,
            lote.cooperativa,
            EcolLoteError::EntregaDeOutraCooperativa
        );
        require!(
            entrega.lote == Pubkey::default(),
            EcolLoteError::EntregaJaVinculada
        );
        require!(
            entrega.material == lote.material,
            EcolLoteError::MaterialDiferente
        );
        require!(
            entrega.variacao == lote.variacao,
            EcolLoteError::VariacaoDiferente
        );

        entrega.lote = lote_key;
        lote.qtd_garrafas = lote
            .qtd_garrafas
            .checked_add(entrega.qtd_garrafas)
            .ok_or(EcolLoteError::Overflow)?;
        lote.qtd_entregas = lote
            .qtd_entregas
            .checked_add(1)
            .ok_or(EcolLoteError::Overflow)?;
        lote.peso_entregas_g = lote
            .peso_entregas_g
            .checked_add(entrega.peso_g)
            .ok_or(EcolLoteError::Overflow)?;
        if !entrega.com_coletor() {
            lote.peso_sem_coletor_g = lote
                .peso_sem_coletor_g
                .checked_add(entrega.peso_g)
                .ok_or(EcolLoteError::Overflow)?;
        }
        lote.entregas_hash = hashv(&[
            &lote.entregas_hash,
            conta.key.as_ref(),
            &entrega.peso_g.to_le_bytes(),
        ])
        .to_bytes();
        let seq = lote.proximo_seq()?;
        emit_cpi!(EntregaNoLote {
            lote: lote_key,
            seq,
            entrega: conta.key(),
            origem: entrega.origem,
            origem_ref: entrega.origem_ref,
            peso_g: entrega.peso_g,
        });
        entrega.exit(&crate::ID)?;
    }
    Ok(())
}

/// Lê uma entrega de `remaining_accounts`: gravável, do programa, na PDA certa.
pub(crate) fn ler_entrega<'info>(
    conta: &'info AccountInfo<'info>,
) -> Result<Account<'info, Entrega>> {
    require!(conta.is_writable, EcolLoteError::EntregaInvalida);
    // Owner e discriminador conferidos pelo `Account`; a PDA confere que é a entrega da cooperativa.
    let entrega: Account<Entrega> =
        Account::try_from(conta).map_err(|_| error!(EcolLoteError::EntregaInvalida))?;
    // Com o bump gravado, recalcular a PDA custa pouco (sem a busca do find_program_address).
    let esperada = Pubkey::create_program_address(
        &[
            SEED_ENTREGA,
            entrega.cooperativa.as_ref(),
            &entrega.entrega_id.to_le_bytes(),
            &[entrega.bump],
        ],
        &crate::ID,
    )
    .map_err(|_| error!(EcolLoteError::EntregaInvalida))?;
    require_keys_eq!(conta.key(), esperada, EcolLoteError::EntregaInvalida);
    Ok(entrega)
}
