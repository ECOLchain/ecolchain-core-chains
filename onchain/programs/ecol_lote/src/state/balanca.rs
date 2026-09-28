use anchor_lang::prelude::*;

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
    pub _reserved: [u8; 32],
}
