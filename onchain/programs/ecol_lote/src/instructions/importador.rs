//! Logística reversa do importador (ADR 0011): garrafas postas no mercado (`Distribuicao`) e coletas
//! que viram lotes de origem na cooperativa ou Clean Tech; a NF da reciclagem fecha a coleta.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        SEED_CARTEIRA, SEED_COLETA, SEED_DISTRIBUICAO, SEED_GLOBAL, SEED_MATERIAL,
        SEED_PARTICIPANTE, SEED_VARIACAO,
    },
    error::EcolLoteError,
    events::{ColetaAtualizada, DistribuicaoRegistrada},
    state::{
        exigir_assinatura, Carteira, Coleta, Distribuicao, Entrega, EstadoColeta, EstadoLote,
        GlobalConfig, Lote, Material, MaterialVariacao, Papel, Participante,
    },
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosDistribuicao {
    pub distrib_id: u64,
    pub material: u16,
    pub variacao: u8,
    pub qtd_garrafas: u32,
    pub peso_estimado_g: u64,
    /// Hash da NF/DI de importação (texto normalizado); nunca o texto.
    pub doc_hash: [u8; 32],
    /// Data de emissão da NF/DI (unix); não pode estar no futuro.
    pub emitido_em: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DadosColeta {
    pub coleta_id: u64,
    pub material: u16,
    pub variacao: u8,
    pub qtd_garrafas: u32,
    pub peso_estimado_g: u64,
    /// Hash do ponto de coleta (LGPD).
    pub local_ref: [u8; 32],
    /// Data da coleta (unix); não pode estar no futuro.
    pub coletado_em: i64,
}

/// Confere material, variação e garrafas de um registro do importador; devolve a variação.
fn conferir_material(
    material: &Material,
    variacao: Option<&MaterialVariacao>,
    variacao_informada: u8,
    qtd_garrafas: u32,
) -> Result<()> {
    material.exigir_ativo()?;
    let v = material.variacao_do_lote(variacao)?;
    require!(v == variacao_informada, EcolLoteError::VariacaoInvalida);
    // O importador fala em garrafas: o material tem de contá-las e a quantidade não pode ser zero.
    require!(qtd_garrafas > 0, EcolLoteError::GarrafasInvalidas);
    material.exigir_garrafas(qtd_garrafas)
}

/// Data declarada pelo importador: positiva e não futura (o registro pode vir depois do fato).
fn exigir_data_passada(data: i64, agora: i64) -> Result<()> {
    require!(data > 0 && data <= agora, EcolLoteError::DataInvalida);
    Ok(())
}

/// Registra garrafas postas no mercado pelo importador.
#[event_cpi]
#[derive(Accounts)]
#[instruction(dados: DadosDistribuicao)]
pub struct ImportadorCreateDistribuicao<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: carteira titular do importador; conferida pelo cadastro e pela assinatura.
    pub importador: UncheckedAccount<'info>,
    pub importador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, importador_assinante.key().as_ref()],
        bump = importador_carteira.bump
    )]
    pub importador_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, importador.key().as_ref()], bump = importador_part.bump)]
    pub importador_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [SEED_MATERIAL, &dados.material.to_le_bytes()], bump = material_cadastro.bump)]
    pub material_cadastro: Box<Account<'info, Material>>,
    #[account(
        seeds = [SEED_VARIACAO, &dados.material.to_le_bytes(), &[dados.variacao]],
        bump = variacao_cadastro.bump
    )]
    pub variacao_cadastro: Option<Box<Account<'info, MaterialVariacao>>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Distribuicao::INIT_SPACE,
        seeds = [SEED_DISTRIBUICAO, importador.key().as_ref(), &dados.distrib_id.to_le_bytes()],
        bump
    )]
    pub distribuicao: Box<Account<'info, Distribuicao>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_importador_create_distribuicao(
    ctx: Context<ImportadorCreateDistribuicao>,
    dados: DadosDistribuicao,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let importador = ctx.accounts.importador.key();
    exigir_assinatura(
        &importador,
        &ctx.accounts.importador_assinante.key(),
        ctx.accounts.importador_carteira.as_deref().map(|c| &**c),
    )?;
    ctx.accounts.importador_part.exigir(Papel::Importador)?;
    conferir_material(
        &ctx.accounts.material_cadastro,
        ctx.accounts.variacao_cadastro.as_deref().map(|v| &**v),
        dados.variacao,
        dados.qtd_garrafas,
    )?;
    require!(dados.doc_hash != [0; 32], EcolLoteError::ReferenciaVazia);
    let agora = Clock::get()?.unix_timestamp;
    exigir_data_passada(dados.emitido_em, agora)?;

    let chave = ctx.accounts.distribuicao.key();
    let d = &mut ctx.accounts.distribuicao;
    d.importador = importador;
    d.distrib_id = dados.distrib_id;
    d.material = dados.material;
    d.variacao = dados.variacao;
    d.qtd_garrafas = dados.qtd_garrafas;
    d.peso_estimado_g = dados.peso_estimado_g;
    d.doc_hash = dados.doc_hash;
    d.emitido_em = dados.emitido_em;
    d.criado_em = agora;
    d.bump = ctx.bumps.distribuicao;
    emit_cpi!(DistribuicaoRegistrada {
        distribuicao: chave,
        importador,
        distrib_id: dados.distrib_id,
        material: dados.material,
        variacao: dados.variacao,
        qtd_garrafas: dados.qtd_garrafas,
        peso_estimado_g: dados.peso_estimado_g,
        doc_hash: dados.doc_hash,
        emitido_em: dados.emitido_em,
        ts: agora,
    });
    Ok(())
}

/// Registra garrafas recolhidas pelo importador (ainda com ele).
#[event_cpi]
#[derive(Accounts)]
#[instruction(dados: DadosColeta)]
pub struct ImportadorRegisterColeta<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: carteira titular do importador; conferida pelo cadastro e pela assinatura.
    pub importador: UncheckedAccount<'info>,
    pub importador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, importador_assinante.key().as_ref()],
        bump = importador_carteira.bump
    )]
    pub importador_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, importador.key().as_ref()], bump = importador_part.bump)]
    pub importador_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(seeds = [SEED_MATERIAL, &dados.material.to_le_bytes()], bump = material_cadastro.bump)]
    pub material_cadastro: Box<Account<'info, Material>>,
    #[account(
        seeds = [SEED_VARIACAO, &dados.material.to_le_bytes(), &[dados.variacao]],
        bump = variacao_cadastro.bump
    )]
    pub variacao_cadastro: Option<Box<Account<'info, MaterialVariacao>>>,
    #[account(
        init,
        payer = payer,
        space = 8 + Coleta::INIT_SPACE,
        seeds = [SEED_COLETA, importador.key().as_ref(), &dados.coleta_id.to_le_bytes()],
        bump
    )]
    pub coleta: Box<Account<'info, Coleta>>,
    pub system_program: Program<'info, System>,
}

pub fn handle_importador_register_coleta(
    ctx: Context<ImportadorRegisterColeta>,
    dados: DadosColeta,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let importador = ctx.accounts.importador.key();
    let ator = ctx.accounts.importador_assinante.key();
    exigir_assinatura(
        &importador,
        &ator,
        ctx.accounts.importador_carteira.as_deref().map(|c| &**c),
    )?;
    ctx.accounts.importador_part.exigir(Papel::Importador)?;
    conferir_material(
        &ctx.accounts.material_cadastro,
        ctx.accounts.variacao_cadastro.as_deref().map(|v| &**v),
        dados.variacao,
        dados.qtd_garrafas,
    )?;
    require!(dados.local_ref != [0; 32], EcolLoteError::ReferenciaVazia);
    let agora = Clock::get()?.unix_timestamp;
    exigir_data_passada(dados.coletado_em, agora)?;

    let chave = ctx.accounts.coleta.key();
    let c = &mut ctx.accounts.coleta;
    c.importador = importador;
    c.coleta_id = dados.coleta_id;
    c.material = dados.material;
    c.variacao = dados.variacao;
    c.qtd_garrafas = dados.qtd_garrafas;
    c.peso_estimado_g = dados.peso_estimado_g;
    c.local_ref = dados.local_ref;
    c.estado = EstadoColeta::Coletada;
    c.coletado_em = dados.coletado_em;
    c.criado_em = agora;
    c.atualizado_em = agora;
    c.bump = ctx.bumps.coleta;
    emit_cpi!(ColetaAtualizada {
        coleta: chave,
        importador,
        de: None,
        para: EstadoColeta::Coletada,
        destino: Pubkey::default(),
        entrega: Pubkey::default(),
        nf_reciclagem_hash: [0; 32],
        ator,
        ts: agora,
    });
    Ok(())
}

/// Contas comuns às etapas seguintes da coleta (enviar, cancelar, informar reciclagem).
#[event_cpi]
#[derive(Accounts)]
pub struct ImportadorEntregarColeta<'info> {
    /// CHECK: carteira titular do importador; conferida contra a coleta e pela assinatura.
    pub importador: UncheckedAccount<'info>,
    pub importador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, importador_assinante.key().as_ref()],
        bump = importador_carteira.bump
    )]
    pub importador_carteira: Option<Account<'info, Carteira>>,
    #[account(seeds = [SEED_PARTICIPANTE, importador.key().as_ref()], bump = importador_part.bump)]
    pub importador_part: Account<'info, Participante>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_COLETA, importador.key().as_ref(), &coleta.coleta_id.to_le_bytes()],
        bump = coleta.bump,
        has_one = importador @ EcolLoteError::CarteiraNaoAutorizada
    )]
    pub coleta: Account<'info, Coleta>,
    /// Cooperativa ou Clean Tech que vai receber as garrafas.
    #[account(
        seeds = [SEED_PARTICIPANTE, destino_part.carteira.as_ref()],
        bump = destino_part.bump
    )]
    pub destino_part: Account<'info, Participante>,
}

/// Envia a coleta a uma cooperativa ou Clean Tech; ela registra o lote de origem ao pesar.
pub fn handle_importador_entregar_coleta(ctx: Context<ImportadorEntregarColeta>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let importador = ctx.accounts.importador.key();
    let ator = ctx.accounts.importador_assinante.key();
    exigir_assinatura(
        &importador,
        &ator,
        ctx.accounts.importador_carteira.as_deref(),
    )?;
    ctx.accounts.importador_part.exigir(Papel::Importador)?;
    ctx.accounts.destino_part.exigir_recebedor()?;
    let destino = ctx.accounts.destino_part.carteira;
    let agora = Clock::get()?.unix_timestamp;
    let chave = ctx.accounts.coleta.key();
    let c = &mut ctx.accounts.coleta;
    c.exigir_estado(EstadoColeta::Coletada)?;
    c.estado = EstadoColeta::EmEntrega;
    c.destino = destino;
    c.atualizado_em = agora;
    emit_cpi!(ColetaAtualizada {
        coleta: chave,
        importador,
        de: Some(EstadoColeta::Coletada),
        para: EstadoColeta::EmEntrega,
        destino,
        entrega: Pubkey::default(),
        nf_reciclagem_hash: [0; 32],
        ator,
        ts: agora,
    });
    Ok(())
}

/// Desfaz o envio (o destino recusou ou o importador desistiu): a coleta volta a `Coletada`.
#[event_cpi]
#[derive(Accounts)]
pub struct ImportadorCancelarEntrega<'info> {
    /// CHECK: carteira titular do importador; conferida contra a coleta e pela assinatura.
    pub importador: UncheckedAccount<'info>,
    pub importador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, importador_assinante.key().as_ref()],
        bump = importador_carteira.bump
    )]
    pub importador_carteira: Option<Account<'info, Carteira>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_COLETA, importador.key().as_ref(), &coleta.coleta_id.to_le_bytes()],
        bump = coleta.bump,
        has_one = importador @ EcolLoteError::CarteiraNaoAutorizada
    )]
    pub coleta: Account<'info, Coleta>,
}

pub fn handle_importador_cancelar_entrega(ctx: Context<ImportadorCancelarEntrega>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let importador = ctx.accounts.importador.key();
    let ator = ctx.accounts.importador_assinante.key();
    exigir_assinatura(
        &importador,
        &ator,
        ctx.accounts.importador_carteira.as_deref(),
    )?;
    let agora = Clock::get()?.unix_timestamp;
    let chave = ctx.accounts.coleta.key();
    let c = &mut ctx.accounts.coleta;
    c.exigir_estado(EstadoColeta::EmEntrega)?;
    c.estado = EstadoColeta::Coletada;
    c.destino = Pubkey::default();
    c.atualizado_em = agora;
    emit_cpi!(ColetaAtualizada {
        coleta: chave,
        importador,
        de: Some(EstadoColeta::EmEntrega),
        para: EstadoColeta::Coletada,
        destino: Pubkey::default(),
        entrega: Pubkey::default(),
        nf_reciclagem_hash: [0; 32],
        ator,
        ts: agora,
    });
    Ok(())
}

/// Anexa a NF da reciclagem: o lote de venda que recebeu a coleta já foi reciclado.
#[event_cpi]
#[derive(Accounts)]
pub struct ImportadorInformarReciclagem<'info> {
    /// CHECK: carteira titular do importador; conferida contra a coleta e pela assinatura.
    pub importador: UncheckedAccount<'info>,
    pub importador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, importador_assinante.key().as_ref()],
        bump = importador_carteira.bump
    )]
    pub importador_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_COLETA, importador.key().as_ref(), &coleta.coleta_id.to_le_bytes()],
        bump = coleta.bump,
        has_one = importador @ EcolLoteError::CarteiraNaoAutorizada,
        has_one = entrega @ EcolLoteError::EntregaInvalida
    )]
    pub coleta: Box<Account<'info, Coleta>>,
    pub entrega: Box<Account<'info, Entrega>>,
    #[account(address = entrega.lote @ EcolLoteError::LoteNaoReciclado)]
    pub lote: Box<Account<'info, Lote>>,
}

pub fn handle_importador_informar_reciclagem(
    ctx: Context<ImportadorInformarReciclagem>,
    nf_reciclagem_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    let importador = ctx.accounts.importador.key();
    let ator = ctx.accounts.importador_assinante.key();
    exigir_assinatura(
        &importador,
        &ator,
        ctx.accounts.importador_carteira.as_deref().map(|c| &**c),
    )?;
    require!(
        nf_reciclagem_hash != [0; 32],
        EcolLoteError::ReferenciaVazia
    );
    require!(
        matches!(
            ctx.accounts.lote.estado,
            EstadoLote::Reciclado | EstadoLote::Agregado | EstadoLote::VendidoFora
        ),
        EcolLoteError::LoteNaoReciclado
    );
    let agora = Clock::get()?.unix_timestamp;
    let chave = ctx.accounts.coleta.key();
    let c = &mut ctx.accounts.coleta;
    c.exigir_estado(EstadoColeta::Entregue)?;
    c.estado = EstadoColeta::Reciclada;
    c.nf_reciclagem_hash = nf_reciclagem_hash;
    c.atualizado_em = agora;
    emit_cpi!(ColetaAtualizada {
        coleta: chave,
        importador,
        de: Some(EstadoColeta::Entregue),
        para: EstadoColeta::Reciclada,
        destino: c.destino,
        entrega: c.entrega,
        nf_reciclagem_hash,
        ator,
        ts: agora,
    });
    Ok(())
}
