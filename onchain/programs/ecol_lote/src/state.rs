pub mod balanca;
pub mod carteira;
pub mod coleta;
pub mod config;
pub mod entrega;
pub mod lote;
pub mod material;
pub mod participante;

pub use balanca::*;
pub use carteira::*;
pub use coleta::*;
pub use config::*;
pub use entrega::*;
pub use lote::*;
pub use material::*;
pub use participante::*;

use anchor_lang::prelude::*;

use crate::{constants::NOME_FIXO_MAX, error::EcolLoteError};

/// Nome de exibição em UTF-8, completado com zeros (público). Contas antigas: todo zero = sem nome.
pub type NomeFixo = [u8; NOME_FIXO_MAX];

/// Valida o nome (não vazio, até 32 bytes, sem `\0`) e o converte para o campo de tamanho fixo.
pub fn codificar_nome(nome: &str) -> Result<NomeFixo> {
    require!(
        !nome.trim().is_empty() && nome.len() <= NOME_FIXO_MAX && !nome.contains('\0'),
        EcolLoteError::NomeInvalido
    );
    let mut campo = [0u8; NOME_FIXO_MAX];
    campo[..nome.len()].copy_from_slice(nome.as_bytes());
    Ok(campo)
}
