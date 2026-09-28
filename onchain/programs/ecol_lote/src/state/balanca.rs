use anchor_lang::prelude::*;

use crate::constants::NOME_FIXO_MAX;

/// Balança cadastrada (PDA `["balanca", dispositivo]`: um equipamento pertence a um único dono). O equipamento tem keypair próprio
/// e assina cada pesagem; o programa confere a assinatura Ed25519.
#[account]
#[derive(InitSpace)]
pub struct Balanca {
    /// Cooperativa ou indústria onde a balança está instalada.
    pub dono: Pubkey,
    /// Chave pública Ed25519 do equipamento.
    pub dispositivo: Pubkey,
    pub ativa: bool,
    pub bump: u8,
    /// Nome de exibição, ex.: "Balança do galpão 2" (ver `NomeFixo`).
    pub nome: [u8; NOME_FIXO_MAX],
}
