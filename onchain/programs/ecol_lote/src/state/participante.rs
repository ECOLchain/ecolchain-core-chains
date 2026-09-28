use anchor_lang::prelude::*;

use crate::{constants::PARTICIPANTE_NOME_MAX, error::EcolLoteError};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum Papel {
    Coletor,
    Cooperativa,
    Transportador,
    Industria,
}

/// Cadastro de uma carteira na EcolChain (PDA `["part", carteira]`). Uma carteira tem um único papel.
#[account]
#[derive(InitSpace)]
pub struct Participante {
    pub carteira: Pubkey,
    pub papel: Papel,
    pub ativo: bool,
    /// Hash do cadastro/KYC mantido off-chain. Nenhum documento vai on-chain (LGPD).
    pub kyc_hash: [u8; 32],
    pub bump: u8,
    /// Nome de exibição em UTF-8, completado com zeros (público). Contas antigas: todo zero = sem nome.
    pub nome: [u8; PARTICIPANTE_NOME_MAX],
}

impl Participante {
    /// Valida e converte o nome para o campo de tamanho fixo.
    pub fn codificar_nome(nome: &str) -> Result<[u8; PARTICIPANTE_NOME_MAX]> {
        require!(
            !nome.trim().is_empty() && nome.len() <= PARTICIPANTE_NOME_MAX && !nome.contains('\0'),
            EcolLoteError::NomeInvalido
        );
        let mut campo = [0u8; PARTICIPANTE_NOME_MAX];
        campo[..nome.len()].copy_from_slice(nome.as_bytes());
        Ok(campo)
    }
}
