use anchor_lang::prelude::*;

/// Entrega de material de um coletor na cooperativa (PDA `["entrega", cooperativa, entrega_id]`),
/// pesada e assinada pela balança da cooperativa. Entra em no máximo um lote.
#[account]
#[derive(InitSpace)]
pub struct Entrega {
    pub cooperativa: Pubkey,
    pub entrega_id: u64,
    /// Referência ao coletor como hash com salt (LGPD): identifica sem expor a carteira.
    pub coletor_ref: [u8; 32],
    /// Código do material cadastrado.
    pub material: u16,
    pub peso_g: u64,
    /// Balança (PDA) que atestou a pesagem.
    pub balanca: Pubkey,
    pub ts_pesagem: i64,
    /// Lote que recebeu a entrega; `Pubkey::default()` enquanto disponível.
    pub lote: Pubkey,
    pub criado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}
