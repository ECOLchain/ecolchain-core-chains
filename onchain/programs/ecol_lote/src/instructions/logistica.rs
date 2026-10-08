use anchor_lang::prelude::*;

use crate::{
    ativo::{self, ContasAsset},
    constants::{
        SEED_AUTORIDADE_ASSET, SEED_BALANCA, SEED_CARTEIRA, SEED_GLOBAL, SEED_LOTE,
        SEED_PARTICIPANTE,
    },
    error::EcolLoteError,
    events::{EscrowMovimentado, LoteTransicionado, MovimentoEscrow, PesagemAtestada},
    pesagem::{self, TipoPesagem},
    state::{
        exigir_assinatura, Balanca, Carteira, GlobalConfig, Lote, ModoRetirada, Papel, Participante,
    },
};

/// Contas para mover o Core asset do lote junto com a custódia.
#[derive(Accounts)]
pub struct ContasCustodia<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: endereço conferido contra `lote.asset` na instrução; o Core valida dono e plugins.
    #[account(mut)]
    pub asset: UncheckedAccount<'info>,
    /// CHECK: PDA sem dados, só assina como autoridade do asset; endereço fixado pelas seeds.
    #[account(seeds = [SEED_AUTORIDADE_ASSET], bump)]
    pub autoridade_asset: UncheckedAccount<'info>,
    /// CHECK: endereço fixo do programa Metaplex Core.
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> ContasCustodia<'info> {
    fn transferir_para(
        &self,
        lote: &Lote,
        novo_dono: &AccountInfo<'info>,
        autoridade_bump: u8,
    ) -> Result<()> {
        require_keys_eq!(self.asset.key(), lote.asset, EcolLoteError::AssetIncorreto);
        let contas = ContasAsset {
            mpl_core_program: &self.mpl_core_program,
            asset: &self.asset,
            autoridade_asset: &self.autoridade_asset,
            payer: &self.payer,
            system_program: &self.system_program,
        };
        ativo::transferir(&contas, novo_dono, autoridade_bump)
    }
}

/// Retirada do lote: transportador e cooperativa assinam juntos a passagem de custódia.
#[event_cpi]
#[derive(Accounts)]
pub struct TransportadorPickupLote<'info> {
    /// CHECK: carteira titular do transportador; conferida pelo cadastro e pela assinatura.
    pub transportador: UncheckedAccount<'info>,
    pub transportador_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, transportador_assinante.key().as_ref()],
        bump = transportador_carteira.bump
    )]
    pub transportador_carteira: Option<Box<Account<'info, Carteira>>>,
    #[account(seeds = [SEED_PARTICIPANTE, transportador.key().as_ref()], bump = transportador_part.bump)]
    pub transportador_part: Box<Account<'info, Participante>>,
    /// CHECK: carteira titular da cooperativa; conferida pelo cadastro/lote e pela assinatura.
    pub cooperativa: UncheckedAccount<'info>,
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
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = cooperativa @ EcolLoteError::NaoEhCooperativaDoLote
    )]
    pub lote: Box<Account<'info, Lote>>,
    pub custodia: ContasCustodia<'info>,
}

pub fn handle_transportador_pickup_lote(ctx: Context<TransportadorPickupLote>) -> Result<()> {
    ctx.accounts.config.exigir_modo_normal()?;
    exigir_assinatura(
        &ctx.accounts.transportador.key(),
        &ctx.accounts.transportador_assinante.key(),
        ctx.accounts.transportador_carteira.as_deref().map(|c| &**c),
    )?;
    exigir_assinatura(
        &ctx.accounts.cooperativa.key(),
        &ctx.accounts.cooperativa_assinante.key(),
        ctx.accounts.cooperativa_carteira.as_deref().map(|c| &**c),
    )?;
    // Retirada própria (venda direta, ADR 0012): quem retira é a indústria compradora.
    if ctx.accounts.lote.modo_retirada == ModoRetirada::Propria {
        require_keys_eq!(
            ctx.accounts.transportador.key(),
            ctx.accounts.lote.industria,
            EcolLoteError::RetiradaDeOutroModo
        );
        ctx.accounts.transportador_part.exigir(Papel::Industria)?;
    } else {
        ctx.accounts
            .transportador_part
            .exigir(Papel::Transportador)?;
    }
    ctx.accounts.cooperativa_part.exigir_recebedor()?;
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.transportador.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.iniciar_transporte()?;
    lote.transportador = ator;
    let (de, seq) = lote.transicionar(para, agora)?;
    ctx.accounts.custodia.transferir_para(
        &ctx.accounts.lote,
        &ctx.accounts.transportador,
        ctx.bumps.custodia.autoridade_asset,
    )?;
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

/// Indústria confirma o recebimento. Se o peso divergir além da tolerância, o lote vai direto para disputa.
/// Com `balanca`, o peso vem assinado pela balança da indústria (instrução Ed25519 imediatamente antes).
#[event_cpi]
#[derive(Accounts)]
pub struct IndustriaConfirmRecebimento<'info> {
    /// CHECK: carteira titular da indústria; conferida contra o lote e pela assinatura.
    pub industria: UncheckedAccount<'info>,
    pub industria_assinante: Signer<'info>,
    #[account(
        seeds = [SEED_CARTEIRA, industria_assinante.key().as_ref()],
        bump = industria_carteira.bump
    )]
    pub industria_carteira: Option<Account<'info, Carteira>>,
    #[account(
        seeds = [SEED_BALANCA, balanca.dispositivo.as_ref()],
        bump = balanca.bump
    )]
    pub balanca: Option<Account<'info, Balanca>>,
    /// CHECK: endereço fixo do sysvar de instruções.
    #[account(address = solana_instructions_sysvar::ID)]
    pub instrucoes_sysvar: UncheckedAccount<'info>,
    #[account(seeds = [SEED_GLOBAL], bump = config.bump)]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump,
        has_one = industria @ EcolLoteError::NaoEhIndustriaDoLote
    )]
    pub lote: Account<'info, Lote>,
    pub custodia: ContasCustodia<'info>,
}

pub fn handle_industria_confirm_recebimento(
    ctx: Context<IndustriaConfirmRecebimento>,
    peso_recebido_g: u64,
    ts_pesagem: Option<i64>,
) -> Result<()> {
    let config = &ctx.accounts.config;
    config.exigir_modo_normal()?;
    exigir_assinatura(
        &ctx.accounts.industria.key(),
        &ctx.accounts.industria_assinante.key(),
        ctx.accounts.industria_carteira.as_deref(),
    )?;
    require!(peso_recebido_g > 0, EcolLoteError::PesoInvalido);
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.industria.key();
    // Sem checar `ativo`: o recebimento encerra um negócio cujo dinheiro já está no escrow.

    // Pesagem atestada é opcional, mas balança e timestamp vêm juntos.
    let atestada = match (&ctx.accounts.balanca, ts_pesagem) {
        (Some(balanca), Some(ts)) => {
            balanca.exigir_ativa_de(&ator)?;
            pesagem::exigir_pesagem_recente(ts, agora)?;
            pesagem::verificar_assinatura_balanca(
                &ctx.accounts.instrucoes_sysvar,
                &balanca.dispositivo,
                &pesagem::mensagem(TipoPesagem::Recebimento, &lote_key, peso_recebido_g, ts),
            )?;
            Some((balanca.key(), ts))
        }
        (None, None) => None,
        _ => return err!(EcolLoteError::AtestacaoAusente),
    };

    let lote = &mut ctx.accounts.lote;
    let divergente = lote.peso_diverge(peso_recebido_g, lote.tolerancia_peso_bps)?;
    let para = lote.estado.receber(divergente)?;
    lote.peso_recebido_g = peso_recebido_g;
    lote.recebimento_atestado = atestada.is_some();
    let (de, seq) = lote.transicionar(para, agora)?;
    let seq_pesagem = match atestada {
        Some(_) => Some(lote.proximo_seq()?),
        None => None,
    };
    // O material está fisicamente na indústria mesmo se o peso divergir e o lote entrar em disputa.
    ctx.accounts.custodia.transferir_para(
        &ctx.accounts.lote,
        &ctx.accounts.industria,
        ctx.bumps.custodia.autoridade_asset,
    )?;
    emit_cpi!(LoteTransicionado {
        lote: lote_key,
        seq,
        de,
        para,
        ator,
        ts: agora
    });
    if let (Some((balanca, ts_pesagem)), Some(seq)) = (atestada, seq_pesagem) {
        emit_cpi!(PesagemAtestada {
            lote: lote_key,
            seq,
            balanca,
            peso_g: peso_recebido_g,
            ts_pesagem,
            recebimento: true,
        });
    }
    Ok(())
}

/// O intermediador atesta que liberou o escrow em R$ para a cooperativa.
#[event_cpi]
#[derive(Accounts)]
pub struct IntermediadorConfirmLiberacao<'info> {
    pub intermediador: Signer<'info>,
    #[account(
        seeds = [SEED_GLOBAL],
        bump = config.bump,
        has_one = intermediador @ EcolLoteError::NaoEhIntermediador
    )]
    pub config: Account<'info, GlobalConfig>,
    #[account(
        mut,
        seeds = [SEED_LOTE, lote.cooperativa.as_ref(), &lote.lote_id.to_le_bytes()],
        bump = lote.bump
    )]
    pub lote: Account<'info, Lote>,
}

pub fn handle_intermediador_confirm_liberacao(
    ctx: Context<IntermediadorConfirmLiberacao>,
    liberacao_ref_hash: [u8; 32],
) -> Result<()> {
    ctx.accounts.config.exigir_disputas_liberadas()?;
    require!(
        liberacao_ref_hash != [0; 32],
        EcolLoteError::ReferenciaVazia
    );
    let agora = Clock::get()?.unix_timestamp;
    let lote_key = ctx.accounts.lote.key();
    let ator = ctx.accounts.intermediador.key();
    let lote = &mut ctx.accounts.lote;
    let para = lote.estado.liberar_escrow()?;
    let valor_centavos = lote.valor_centavos;
    lote.valor_liberado_centavos = valor_centavos;
    let (de, seq) = lote.transicionar(para, agora)?;
    let seq_escrow = lote.proximo_seq()?;
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
        seq: seq_escrow,
        movimento: MovimentoEscrow::Liberacao,
        ref_hash: liberacao_ref_hash,
        valor_centavos,
    });
    Ok(())
}
