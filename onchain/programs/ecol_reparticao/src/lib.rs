//! EcolChain — repartição da receita dos créditos de carbono.
//!
//! Para cada crédito vendido, registra os direitos de cada participante (árvore Merkle, modelo
//! pull) e cada resgate em cupons feito pela Zupy. O dinheiro (R$) circula fora da blockchain;
//! aqui ficam os tetos por papel, calculados on-chain, e a trilha de quem recebeu o quê.

pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod merkle;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K");

#[program]
pub mod ecol_reparticao {
    use super::*;

    pub fn admin_init_config(
        ctx: Context<AdminInitConfig>,
        admin: Pubkey,
        params: ParamsReparticao,
    ) -> Result<()> {
        instructions::admin::handle_admin_init_config(ctx, admin, params)
    }

    pub fn admin_update_config(
        ctx: Context<AdminUpdateConfig>,
        params: ParamsReparticao,
    ) -> Result<()> {
        instructions::admin::handle_admin_update_config(ctx, params)
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

    pub fn operador_create_distribuicao(
        ctx: Context<OperadorCreateDistribuicao>,
        bps_industria: u16,
        merkle_root: [u8; 32],
        num_folhas: u32,
    ) -> Result<()> {
        instructions::direitos::handle_operador_create_distribuicao(
            ctx,
            bps_industria,
            merkle_root,
            num_folhas,
        )
    }

    pub fn intermediador_record_repasse(
        ctx: Context<IntermediadorRecordRepasse>,
        zupy_ref_hash: [u8; 32],
        ecolchain_ref_hash: [u8; 32],
    ) -> Result<()> {
        instructions::direitos::handle_intermediador_record_repasse(
            ctx,
            zupy_ref_hash,
            ecolchain_ref_hash,
        )
    }

    pub fn zupy_record_resgate(
        ctx: Context<ZupyRecordResgate>,
        indice: u32,
        papel: ecol_lote::Papel,
        valor_centavos: u64,
        prova: Vec<[u8; 32]>,
        recibo_hash: [u8; 32],
    ) -> Result<()> {
        instructions::direitos::handle_zupy_record_resgate(
            ctx,
            indice,
            papel,
            valor_centavos,
            prova,
            recibo_hash,
        )
    }
}
