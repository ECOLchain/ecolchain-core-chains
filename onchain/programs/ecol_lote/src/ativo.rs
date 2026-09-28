//! Core asset do lote (Metaplex Core), usado só como recibo visível em carteiras.
//! O estado canônico continua na PDA `Lote`.
//!
//! O asset nasce congelado (`PermanentFreezeDelegate`) e só a PDA `autoridade_asset` do programa
//! o transfere (`PermanentTransferDelegate`), acompanhando a custódia: cooperativa → transportador → indústria.
//! Desde a ADR 0010 também o queima (`PermanentBurnDelegate`) quando a união do lote de venda é desfeita.

use anchor_lang::prelude::*;
use mpl_core::{
    instructions::{BurnV1CpiBuilder, CreateV2CpiBuilder, TransferV1CpiBuilder},
    types::{
        DataState, PermanentBurnDelegate, PermanentFreezeDelegate, PermanentTransferDelegate,
        Plugin, PluginAuthority, PluginAuthorityPair,
    },
};

use crate::constants::{SEED_ASSET, SEED_AUTORIDADE_ASSET};

pub struct ContasAsset<'a, 'info> {
    pub mpl_core_program: &'a AccountInfo<'info>,
    pub asset: &'a AccountInfo<'info>,
    pub autoridade_asset: &'a AccountInfo<'info>,
    pub payer: &'a AccountInfo<'info>,
    pub system_program: &'a AccountInfo<'info>,
}

pub fn criar<'info>(
    contas: &ContasAsset<'_, 'info>,
    dono: &AccountInfo<'info>,
    lote: &Pubkey,
    lote_id: u64,
    uri: String,
    asset_bump: u8,
) -> Result<()> {
    let autoridade = PluginAuthority::Address {
        address: contas.autoridade_asset.key(),
    };
    let seeds_asset: &[&[u8]] = &[SEED_ASSET, lote.as_ref(), &[asset_bump]];
    CreateV2CpiBuilder::new(contas.mpl_core_program)
        .asset(contas.asset)
        .payer(contas.payer)
        .owner(Some(dono))
        .update_authority(Some(contas.autoridade_asset))
        .system_program(contas.system_program)
        .data_state(DataState::AccountState)
        .name(format!("EcolChain lote {lote_id}"))
        .uri(uri)
        .plugins(vec![
            PluginAuthorityPair {
                plugin: Plugin::PermanentFreezeDelegate(PermanentFreezeDelegate { frozen: true }),
                authority: Some(autoridade.clone()),
            },
            PluginAuthorityPair {
                plugin: Plugin::PermanentTransferDelegate(PermanentTransferDelegate {}),
                authority: Some(autoridade.clone()),
            },
            PluginAuthorityPair {
                plugin: Plugin::PermanentBurnDelegate(PermanentBurnDelegate {}),
                authority: Some(autoridade),
            },
        ])
        .invoke_signed(&[seeds_asset])?;
    Ok(())
}

pub fn transferir<'info>(
    contas: &ContasAsset<'_, 'info>,
    novo_dono: &AccountInfo<'info>,
    autoridade_bump: u8,
) -> Result<()> {
    let seeds_autoridade: &[&[u8]] = &[SEED_AUTORIDADE_ASSET, &[autoridade_bump]];
    TransferV1CpiBuilder::new(contas.mpl_core_program)
        .asset(contas.asset)
        .payer(contas.payer)
        .authority(Some(contas.autoridade_asset))
        .new_owner(novo_dono)
        .system_program(Some(contas.system_program))
        .invoke_signed(&[seeds_autoridade])?;
    Ok(())
}

/// Queima o recibo de um lote de venda desfeito; o rent volta para quem paga a transação.
pub fn queimar<'info>(contas: &ContasAsset<'_, 'info>, autoridade_bump: u8) -> Result<()> {
    let seeds_autoridade: &[&[u8]] = &[SEED_AUTORIDADE_ASSET, &[autoridade_bump]];
    BurnV1CpiBuilder::new(contas.mpl_core_program)
        .asset(contas.asset)
        .payer(contas.payer)
        .authority(Some(contas.autoridade_asset))
        .system_program(Some(contas.system_program))
        .invoke_signed(&[seeds_autoridade])?;
    Ok(())
}
