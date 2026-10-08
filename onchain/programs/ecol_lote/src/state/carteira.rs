use anchor_lang::prelude::*;

use crate::{constants::NOME_FIXO_MAX, error::EcolLoteError};

/// Carteira vinculada a um participante (PDA `["carteira", endereco]`, ADR 0011). Assina pelo ator;
/// a identidade do ator continua sendo a carteira titular (`Participante.carteira`), usada nas seeds
/// dos lotes e nos campos `cooperativa`/`industria`/`transportador`.
/// Uma carteira tem no máximo um vínculo (a PDA é derivada só do endereço). A do próprio titular
/// também pode existir, só para dar nome a ela.
#[account]
#[derive(InitSpace)]
pub struct Carteira {
    pub endereco: Pubkey,
    /// Carteira titular do participante.
    pub participante: Pubkey,
    pub nome: [u8; NOME_FIXO_MAX],
    pub ativa: bool,
    pub criada_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
}

/// Confere que `assinante` fala por `titular`: é o próprio titular ou uma carteira ativa vinculada a ele.
/// `carteira`, quando presente, já teve as seeds `["carteira", assinante]` conferidas pelo Anchor.
pub fn exigir_assinatura(
    titular: &Pubkey,
    assinante: &Pubkey,
    carteira: Option<&Carteira>,
) -> Result<()> {
    if assinante == titular {
        return Ok(());
    }
    match carteira {
        Some(c) if c.endereco == *assinante && c.participante == *titular && c.ativa => Ok(()),
        _ => err!(EcolLoteError::CarteiraNaoAutorizada),
    }
}
