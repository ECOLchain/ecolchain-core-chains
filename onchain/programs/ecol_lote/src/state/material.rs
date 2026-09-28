use anchor_lang::prelude::*;

use crate::{constants::MATERIAL_NOME_MAX, error::EcolLoteError};

/// Tipo de material reciclável (PDA `["material", codigo]`), cadastrado pelo operador.
/// Lotes e créditos guardam só o `codigo`; o nome serve para exibição.
#[account]
#[derive(InitSpace)]
pub struct Material {
    pub codigo: u16,
    #[max_len(MATERIAL_NOME_MAX)]
    pub nome: String,
    /// Material inativo não entra em lotes novos; lotes existentes não são afetados.
    pub ativo: bool,
    pub bump: u8,
    pub _reserved: [u8; 32],
}

impl Material {
    pub fn validar_nome(nome: &str) -> Result<()> {
        require!(
            !nome.trim().is_empty() && nome.len() <= MATERIAL_NOME_MAX,
            EcolLoteError::NomeInvalido
        );
        Ok(())
    }

    pub fn exigir_ativo(&self) -> Result<()> {
        require!(self.ativo, EcolLoteError::MaterialInativo);
        Ok(())
    }
}
