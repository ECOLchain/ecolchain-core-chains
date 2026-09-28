//! EcolChain — registro do lote de reciclado e sua máquina de estados.
//!
//! O dinheiro (escrow em R$) fica off-chain com um intermediador; este programa registra cada etapa,
//! quem a assinou e as atestações do intermediador, para auditoria.

pub mod ativo;
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod pesagem;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD");

#[program]
pub mod ecol_lote {
    use super::*;

    pub fn admin_init_global(
        ctx: Context<AdminInitGlobal>,
        admin: Pubkey,
        params: ConfigParams,
    ) -> Result<()> {
        instructions::admin::handle_admin_init_global(ctx, admin, params)
    }

    pub fn admin_update_global(
        ctx: Context<AdminUpdateGlobal>,
        params: ConfigParams,
    ) -> Result<()> {
        instructions::admin::handle_admin_update_global(ctx, params)
    }

    pub fn admin_set_modo(ctx: Context<AdminUpdateGlobal>, modo: ModoOperacao) -> Result<()> {
        instructions::admin::handle_admin_set_modo(ctx, modo)
    }

    pub fn admin_set_guardiao(ctx: Context<AdminUpdateGlobal>, guardiao: Pubkey) -> Result<()> {
        instructions::admin::handle_admin_set_guardiao(ctx, guardiao)
    }

    pub fn guardiao_pausar(ctx: Context<GuardiaoPausar>, modo: ModoOperacao) -> Result<()> {
        instructions::admin::handle_guardiao_pausar(ctx, modo)
    }

    pub fn admin_propose_admin(ctx: Context<AdminUpdateGlobal>, novo_admin: Pubkey) -> Result<()> {
        instructions::admin::handle_admin_propose_admin(ctx, novo_admin)
    }

    pub fn admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
        instructions::admin::handle_admin_accept_admin(ctx)
    }

    pub fn operador_register_participante(
        ctx: Context<OperadorRegisterParticipante>,
        carteira: Pubkey,
        papel: Papel,
        kyc_hash: [u8; 32],
    ) -> Result<()> {
        instructions::participantes::handle_operador_register_participante(
            ctx, carteira, papel, kyc_hash,
        )
    }

    pub fn operador_set_participante_ativo(
        ctx: Context<OperadorSetParticipanteAtivo>,
        ativo: bool,
    ) -> Result<()> {
        instructions::participantes::handle_operador_set_participante_ativo(ctx, ativo)
    }

    pub fn operador_register_balanca(
        ctx: Context<OperadorRegisterBalanca>,
        dispositivo: Pubkey,
    ) -> Result<()> {
        instructions::balancas::handle_operador_register_balanca(ctx, dispositivo)
    }

    pub fn operador_set_balanca_ativa(
        ctx: Context<OperadorSetBalancaAtiva>,
        ativa: bool,
    ) -> Result<()> {
        instructions::balancas::handle_operador_set_balanca_ativa(ctx, ativa)
    }

    pub fn operador_create_material(
        ctx: Context<OperadorCreateMaterial>,
        codigo: u16,
        nome: String,
    ) -> Result<()> {
        instructions::materiais::handle_operador_create_material(ctx, codigo, nome)
    }

    pub fn operador_update_material(
        ctx: Context<OperadorUpdateMaterial>,
        nome: String,
        ativo: bool,
    ) -> Result<()> {
        instructions::materiais::handle_operador_update_material(ctx, nome, ativo)
    }

    pub fn cooperativa_register_entrega(
        ctx: Context<CooperativaRegisterEntrega>,
        dados: DadosEntrega,
    ) -> Result<()> {
        instructions::entregas::handle_cooperativa_register_entrega(ctx, dados)
    }

    pub fn cooperativa_add_entregas<'info>(
        ctx: Context<'info, CooperativaAddEntregas<'info>>,
    ) -> Result<()> {
        instructions::entregas::handle_cooperativa_add_entregas(ctx)
    }

    pub fn cooperativa_create_lote(
        ctx: Context<CooperativaCreateLote>,
        dados: DadosLote,
    ) -> Result<()> {
        instructions::lote_criacao::handle_cooperativa_create_lote(ctx, dados)
    }

    pub fn cooperativa_list_lote(
        ctx: Context<CooperativaListLote>,
        prazo_leilao: i64,
        preco_minimo_centavos: u64,
    ) -> Result<()> {
        instructions::marketplace::handle_cooperativa_list_lote(
            ctx,
            prazo_leilao,
            preco_minimo_centavos,
        )
    }

    pub fn cooperativa_cancel_anuncio(ctx: Context<CooperativaListLote>) -> Result<()> {
        instructions::marketplace::handle_cooperativa_cancel_anuncio(ctx)
    }

    pub fn operador_close_leilao_sem_lance(
        ctx: Context<OperadorCloseLeilaoSemLance>,
    ) -> Result<()> {
        instructions::marketplace::handle_operador_close_leilao_sem_lance(ctx)
    }

    pub fn industria_accept_venda(
        ctx: Context<IndustriaAcceptVenda>,
        valor_centavos: u64,
        escrow_ref_hash: [u8; 32],
        lances_root: [u8; 32],
    ) -> Result<()> {
        instructions::marketplace::handle_industria_accept_venda(
            ctx,
            valor_centavos,
            escrow_ref_hash,
            lances_root,
        )
    }

    pub fn transportador_pickup_lote(ctx: Context<TransportadorPickupLote>) -> Result<()> {
        instructions::logistica::handle_transportador_pickup_lote(ctx)
    }

    pub fn industria_confirm_recebimento(
        ctx: Context<IndustriaConfirmRecebimento>,
        peso_recebido_g: u64,
        ts_pesagem: Option<i64>,
    ) -> Result<()> {
        instructions::logistica::handle_industria_confirm_recebimento(
            ctx,
            peso_recebido_g,
            ts_pesagem,
        )
    }

    pub fn intermediador_confirm_liberacao(
        ctx: Context<IntermediadorConfirmLiberacao>,
        liberacao_ref_hash: [u8; 32],
    ) -> Result<()> {
        instructions::logistica::handle_intermediador_confirm_liberacao(ctx, liberacao_ref_hash)
    }

    pub fn parte_open_disputa(ctx: Context<ParteOpenDisputa>, motivo: MotivoDisputa) -> Result<()> {
        instructions::disputa::handle_parte_open_disputa(ctx, motivo)
    }

    pub fn public_crank_prazo_expirado(ctx: Context<PublicCrankPrazoExpirado>) -> Result<()> {
        instructions::disputa::handle_public_crank_prazo_expirado(ctx)
    }

    pub fn arbitro_resolve_disputa(
        ctx: Context<ArbitroResolveDisputa>,
        decisao: DecisaoDisputa,
    ) -> Result<()> {
        instructions::disputa::handle_arbitro_resolve_disputa(ctx, decisao)
    }

    pub fn credito_mark_agregado(ctx: Context<CreditoMarkAgregado>, credito: Pubkey) -> Result<()> {
        instructions::credito::handle_credito_mark_agregado(ctx, credito)
    }

    pub fn credito_mark_desagregado(
        ctx: Context<CreditoMarkAgregado>,
        credito: Pubkey,
    ) -> Result<()> {
        instructions::credito::handle_credito_mark_desagregado(ctx, credito)
    }
}
