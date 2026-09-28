use anchor_lang::prelude::*;

use crate::constants::NOME_FIXO_MAX;

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
    /// Nome de exibição (ver `NomeFixo`).
    pub nome: [u8; NOME_FIXO_MAX],
}
