use anchor_lang::prelude::*;

use crate::{
    constants::{DURACAO_LEILAO_MAX_S, SEED_CARTEIRA, SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE},
    error::EcolLoteError,
    events::{EscrowMovimentado, LoteTransicionado, LoteVendido, MovimentoEscrow},
    state::{exigir_assinatura, Carteira, GlobalConfig, Lote, ModoRetirada, Papel, Participante},
};

/// Anuncia o lote (a partir de `Criado`) ou o reanuncia (a partir de `SemLance`) com novo preço mínimo.
/// As mesmas contas servem para `cooperativa_cancel_anuncio`.
#[event_cpi]
#[derive(Accounts)]
pub struct CooperativaListLote<'info> {
    /// CHECK: carteira titular da cooperativa; conferida pelo cadastro/lote e pela assinatura.
    pub cooperativa: UncheckedAccount<'info>,
    pub cooperativa_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, cooperativa_assinante.key().as_ref()],
        bump = cooperativa_carteira.bump
    )]
    pub cooperativa_carteira: Option<Account<'info, Carteira>>,
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
    ctx.accounts.exigir_assinatura()?;
    // Cooperativa desativada não assume novos compromissos comerciais.
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
    require!(preco_minimo_centavos > 0, EcolLoteError::ParametroInvalido);
    // Só vai a leilão um lote montado com entregas de coletores (base da repartição).
    require!(
        ctx.accounts.lote.qtd_entregas > 0,
        EcolLoteError::LoteSemEntregas
    );
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
    lote.venda_direta = false;
    // Nova rodada: os lances de um anúncio anterior não valem mais (ADR 0013).
    lote.zerar_lances();
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
    ctx.accounts.exigir_assinatura()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    // Com lances, o leilão não é retirado antes do prazo; depois dele, sim (o vencedor pode desistir).
    if lote.qtd_lances > 0 {
        require!(
            agora >= lote.prazo_do_leilao()?,
            EcolLoteError::LeilaoComLances
        );
    }
    let para = lote.estado.cancelar_anuncio()?;
    lote.venda_direta = false;
    lote.zerar_lances();
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

/// Põe o lote à venda por preço fixo, sem leilão nem prazo (ADR 0012): a primeira indústria que pagar
/// o preço (ou mais) leva. Para mudar o preço: `cooperativa_cancel_anuncio` + esta, na mesma transação.
pub fn handle_cooperativa_anunciar_venda(
    ctx: Context<CooperativaListLote>,
    preco_centavos: u64,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.exigir_assinatura()?;
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
    require!(preco_centavos > 0, EcolLoteError::ParametroInvalido);
    require!(
        ctx.accounts.lote.qtd_entregas > 0,
        EcolLoteError::LoteSemEntregas
    );
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    // Sem prazo: o operador não encerra a venda direta como "sem lance".
    let para = lote.estado.anunciar(agora, i64::MAX)?;
    lote.preco_minimo_centavos = preco_centavos;
    lote.venda_direta = true;
    lote.zerar_lances();
    lote.modo_retirada = ModoRetirada::Indefinido;
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

/// Marca o lote como vendido fora da plataforma (sem valor nem comprador), ou desfaz a marcação.
pub fn handle_cooperativa_set_vendido_fora(
    ctx: Context<CooperativaListLote>,
    vendido: bool,
) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    ctx.accounts.exigir_assinatura()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    let para = if vendido {
        lote.estado.marcar_vendido_fora()?
    } else {
        lote.estado.desmarcar_vendido_fora()?
    };
    lote.venda_direta = false;
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

/// Venda direta: depois do recebimento, a cooperativa confirma que recebeu o pagamento (fora da
/// blockchain, comprovante por hash). Faz o papel da liberação do escrow: o lote vai a `Reciclado`.
pub fn handle_cooperativa_confirmar_pagamento(
    ctx: Context<CooperativaListLote>,
    comprovante_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_disputas_liberadas()?;
    ctx.accounts.exigir_assinatura()?;
    require!(comprovante_hash != [0; 32], EcolLoteError::ReferenciaVazia);
    require!(
        ctx.accounts.lote.venda_direta,
        EcolLoteError::LoteNaoEhVendaDireta
    );
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.cooperativa.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.liberar_escrow()?;
    let valor_centavos = lote.valor_centavos;
    lote.valor_liberado_centavos = valor_centavos;
    let (de, seq) = lote.transicionar(para, agora)?;
    let seq_pagamento = lote.proximo_seq()?;
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
        seq: seq_pagamento,
        movimento: MovimentoEscrow::Liberacao,
        ref_hash: comprovante_hash,
        valor_centavos,
    });
    Ok(())
}

impl CooperativaListLote<'_> {
    fn exigir_assinatura(&self) -> Result<()> {
        exigir_assinatura(
            &self.cooperativa.key(),
            &self.cooperativa_assinante.key(),
            self.cooperativa_carteira.as_deref(),
        )
    }
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
    // Com lances on-chain, o leilão termina com a venda ao vencedor (ou a cooperativa o retira).
    require!(lote.qtd_lances == 0, EcolLoteError::LeilaoComLances);
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
    exigir_assinatura(
        &ctx.accounts.industria.key(),
        &ctx.accounts.industria_assinante.key(),
        ctx.accounts.industria_carteira.as_deref().map(|c| &**c),
    )?;
    ctx.accounts.industria_part.exigir(Papel::Industria)?;
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
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
    // A venda direta é comprada pela própria indústria (`industria_comprar_lote`), sem apuração.
    require!(!lote.venda_direta, EcolLoteError::LoteEmVendaDireta);
    require!(
        valor_centavos >= lote.preco_minimo_centavos,
        EcolLoteError::ValorAbaixoDoMinimo
    );
    // Leilão com lances on-chain (ADR 0013): só depois do prazo, ao líder e pelo valor do lance.
    // Sem lances on-chain, vale a apuração off-chain atestada pelo operador.
    if lote.qtd_lances > 0 {
        require!(
            agora >= lote.prazo_do_leilao()?,
            EcolLoteError::LeilaoEmAndamento
        );
        require!(
            lote.lance_lider == industria && lote.maior_lance_centavos == valor_centavos,
            EcolLoteError::NaoEhLanceVencedor
        );
    }
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

/// Compra direta (ADR 0012): a indústria compra sozinha um lote à venda por preço fixo, pagando o preço
/// pedido ou mais, e escolhe quem retira. O pagamento é combinado fora da blockchain.
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaComprarLote<'info> {
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
    #[account(seeds = [SEED_PARTICIPANTE, lote.cooperativa.as_ref()], bump = cooperativa_part.bump)]
    pub cooperativa_part: Box<Account<'info, Participante>>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Box<Account<'info, GlobalConfig>>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Box<Account<'info, Lote>>,
}

pub fn handle_industria_comprar_lote(
    ctx: Context<IndustriaComprarLote>,
    valor_centavos: u64,
    modo_retirada: ModoRetirada,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    exigir_assinatura(
        &ctx.accounts.industria.key(),
        &ctx.accounts.industria_assinante.key(),
        ctx.accounts.industria_carteira.as_deref().map(|c| &**c),
    )?;
    ctx.accounts.industria_part.exigir(Papel::Industria)?;
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
    require!(
        !config.eh_chave_de_papel(&ctx.accounts.industria.key()),
        EcolLoteError::CarteiraReservada
    );
    require!(
        modo_retirada != ModoRetirada::Indefinido,
        EcolLoteError::ModoRetiradaInvalido
    );

    let agora = Clock::get()?.unix_timestamp;
    let prazo_entrega = agora
        .checked_add(config.prazo_entrega_s)
        .ok_or(EcolLoteError::Overflow)?;
    let tolerancia_peso_bps = config.tolerancia_peso_bps;
    let lote_key = ctx.accounts.lote.key();
    let industria = ctx.accounts.industria.key();
    let lote = &mut ctx.accounts.lote;
    require!(lote.venda_direta, EcolLoteError::LoteNaoEhVendaDireta);
    require!(
        valor_centavos >= lote.preco_minimo_centavos,
        EcolLoteError::ValorAbaixoDoMinimo
    );
    let para = lote.estado.vender()?;
    lote.industria = industria;
    lote.valor_centavos = valor_centavos;
    lote.modo_retirada = modo_retirada;
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
    // Sem escrow nem leilão: as referências ficam zeradas.
    emit_cpi!(LoteVendido {
        lote: lote_key,
        seq: seq_venda,
        industria,
        valor_centavos,
        escrow_ref_hash: [0; 32],
        lances_root: [0; 32],
    });
    Ok(())
}
