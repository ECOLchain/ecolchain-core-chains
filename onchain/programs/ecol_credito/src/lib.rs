//! EcolChain — registro do crédito de carbono.
//!
//! O crédito nasce no registro oficial (Verra / Gold Standard). Este programa não o tokeniza:
//! registra quais lotes físicos o originaram (marcando-os `Agregado` no `ecol_lote`), o número
//! de série, a venda e a aposentadoria.

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ");

#[program]
pub mod ecol_credito {
    use super::*;

    pub fn admin_init_config(
        ctx: Context<AdminInitConfig>,
        admin: Pubkey,
        operador: Pubkey,
        registrador: Pubkey,
    ) -> Result<()> {
        instructions::admin::handle_admin_init_config(ctx, admin, operador, registrador)
    }

    pub fn admin_update_config(
        ctx: Context<AdminUpdateConfig>,
        operador: Pubkey,
        registrador: Pubkey,
    ) -> Result<()> {
        instructions::admin::handle_admin_update_config(ctx, operador, registrador)
    }

    pub fn admin_set_pausado(ctx: Context<AdminUpdateConfig>, pausado: bool) -> Result<()> {
        instructions::admin::handle_admin_set_pausado(ctx, pausado)
    }

    pub fn admin_propose_admin(ctx: Context<AdminUpdateConfig>, novo_admin: Pubkey) -> Result<()> {
        instructions::admin::handle_admin_propose_admin(ctx, novo_admin)
    }

    pub fn admin_accept_admin(ctx: Context<AdminAcceptAdmin>) -> Result<()> {
        instructions::admin::handle_admin_accept_admin(ctx)
    }

    pub fn operador_create_credito(
        ctx: Context<OperadorCreateCredito>,
        credito_id: u64,
        padrao: PadraoRegistro,
        material: ecol_lote::Material,
        vintage: u16,
        metodologia_hash: [u8; 32],
    ) -> Result<()> {
        instructions::agregacao::handle_operador_create_credito(
            ctx,
            credito_id,
            padrao,
            material,
            vintage,
            metodologia_hash,
        )
    }

    pub fn operador_add_lotes<'info>(ctx: Context<'info, OperadorAddLotes<'info>>) -> Result<()> {
        instructions::agregacao::handle_operador_add_lotes(ctx)
    }

    pub fn operador_submit_credito(ctx: Context<OperadorSubmitCredito>) -> Result<()> {
        instructions::agregacao::handle_operador_submit_credito(ctx)
    }

    pub fn registrador_record_emissao(
        ctx: Context<RegistradorRecordEmissao>,
        serial: String,
        serial_hash: [u8; 32],
        tco2e_milli: u64,
    ) -> Result<()> {
        instructions::ciclo::handle_registrador_record_emissao(
            ctx,
            serial,
            serial_hash,
            tco2e_milli,
        )
    }

    pub fn registrador_record_rejeicao(
        ctx: Context<RegistradorRecordRejeicao>,
        motivo_hash: [u8; 32],
    ) -> Result<()> {
        instructions::ciclo::handle_registrador_record_rejeicao(ctx, motivo_hash)
    }

    pub fn operador_record_venda(
        ctx: Context<OperadorRecordVenda>,
        valor_centavos: u64,
    ) -> Result<()> {
        instructions::ciclo::handle_operador_record_venda(ctx, valor_centavos)
    }

    pub fn registrador_record_aposentadoria(
        ctx: Context<RegistradorRecordRejeicao>,
        prova_hash: [u8; 32],
    ) -> Result<()> {
        instructions::ciclo::handle_registrador_record_aposentadoria(ctx, prova_hash)
    }
}
