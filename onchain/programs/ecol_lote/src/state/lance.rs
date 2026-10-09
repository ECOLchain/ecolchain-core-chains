use anchor_lang::prelude::*;

/// Lance de uma indústria num leilão (ADR 0013): PDA `["lance", lote, industria]`, uma por indústria e
/// lote. O aumento regrava a mesma conta; um novo anúncio do lote abre outra rodada (`prazo_leilao`),
/// e o lance da rodada anterior deixa de valer até ser aumentado de novo.
#[account]
#[derive(InitSpace)]
pub struct Lance {
    pub lote: Pubkey,
    /// Carteira titular da indústria (a assinante pode ser uma carteira vinculada).
    pub industria: Pubkey,
    pub valor_centavos: u64,
    /// Prazo do leilão em que o lance foi dado: identifica a rodada.
    pub prazo_leilao: i64,
    /// Quem pagou o rent; recebe de volta no `public_close_lance`.
    pub pagador: Pubkey,
    pub criado_em: i64,
    pub atualizado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}
