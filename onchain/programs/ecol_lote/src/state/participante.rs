use anchor_lang::prelude::*;

use crate::constants::NOME_FIXO_MAX;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum Papel {
    Coletor,
    Cooperativa,
    Transportador,
    Industria,
    /// Importador de vinhos: põe garrafas no mercado e faz a logística reversa (ADR 0011).
    Importador,
    /// Recebe, pesa, monta e vende lotes como a cooperativa (ADR 0011).
    CleanTech,
}

impl Papel {
    /// Papéis que recebem material na balança própria, montam lotes de venda e os anunciam.
    pub fn eh_recebedor(self) -> bool {
        matches!(self, Papel::Cooperativa | Papel::CleanTech)
    }
}

/// Cadastro de uma carteira na EcolChain (PDA `["part", carteira]`). Uma carteira tem um único papel;
/// o ator pode ter outras carteiras vinculadas (`Carteira`), que assinam por ele.
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
