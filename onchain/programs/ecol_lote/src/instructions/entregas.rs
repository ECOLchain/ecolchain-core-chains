use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;

use crate::{
    constants::{
        MAX_ENTREGAS_POR_TX, SEED_BALANCA, SEED_ENTREGA, SEED_GLOBAL, SEED_LOTE, SEED_MATERIAL,
        SEED_PARTICIPANTE,
    },
    error::EcolLoteError,
    events::{EntregaNoLote, EntregaRegistrada},
    pesagem::{self, TipoPesagem},
    state::{
        Balanca, Entrega, EstadoLote, GlobalConfig, Lote, Material, OrigemEntrega, Papel,
        Participante,
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
}

/// Registra um lote de origem (com ou sem coletor), pesado pela balança da cooperativa. A transação
/// deve ter, imediatamente antes, a instrução Ed25519 da balança sobre `(domínio, Entrega, entrega, peso, ts)`.
#[event_cpi]
#[derive(Accounts)]
#[instruction(dados: DadosEntrega)]
pub struct CooperativaRegisterEntrega<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub cooperativa: Signer<'info>,
    #[account(seeds = [SEED_PARTICIPANTE, cooperativa.key().as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()], bump = balanca.bump)]
    pub balanca: Box<Account<'info, Balanca>>,
    #[account(seeds = [SEED_MATERIAL, &dados.material.to_le_bytes()], bump = material_cadastro.bump)]
    pub material_cadastro: Box<Account<'info, Material>>,
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
    ctx.accounts.cooperativa_part.exigir(Papel::Cooperativa)?;
    let cooperativa = ctx.accounts.cooperativa.key();
    ctx.accounts.balanca.exigir_ativa_de(&cooperativa)?;
    ctx.accounts.material_cadastro.exigir_ativo()?;
    require!(dados.peso_g > 0, EcolLoteError::PesoInvalido);
    // Toda origem tem referência: coletor, documento da doação/compra ou ticket da triagem.
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
    });
    Ok(())
}

/// Monta o lote de venda: vincula lotes de origem disponíveis (em `remaining_accounts`, graváveis)
/// a um lote ainda `EmMontagem`. Mesma cooperativa, mesmo material; cada origem entra em um só lote.
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaAddEntregas<'info> {
    pub cooperativa: Signer<'info>,
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

        entrega.lote = lote_key;
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
