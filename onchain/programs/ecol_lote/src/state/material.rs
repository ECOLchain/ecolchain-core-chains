use anchor_lang::prelude::*;

use crate::{
    constants::{MATERIAL_NOME_MAX, NOME_FIXO_MAX, SEED_VARIACAO},
    error::EcolLoteError,
};

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
    /// Variações cadastradas (`MaterialVariacao` com índices 1..=qtd). Com alguma, lotes exigem variação.
    pub qtd_variacoes: u8,
    /// Lotes deste material informam a quantidade aproximada de garrafas (vidro).
    pub conta_garrafas: bool,
    pub _reserved: [u8; 30],
}

/// Variação de um material (PDA `["variacao", codigo, indice]`), como a cor do vidro.
/// Lotes guardam o índice; um lote de venda não mistura variações.
#[account]
#[derive(InitSpace)]
pub struct MaterialVariacao {
    pub material: u16,
    pub indice: u8,
    pub nome: [u8; NOME_FIXO_MAX],
    /// Variação inativa não entra em lotes novos.
    pub ativa: bool,
    pub bump: u8,
    pub _reserved: [u8; 16],
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

    /// Confere a variação informada para um lote novo deste material e devolve o índice
    /// (0 quando o material não tem variações). A conta já veio com seeds conferidas pelo Anchor
    /// a partir do índice; aqui só se confere que é deste material e está ativa.
    pub fn variacao_do_lote(&self, variacao: Option<&MaterialVariacao>) -> Result<u8> {
        match (self.qtd_variacoes, variacao) {
            (0, None) => Ok(0),
            (0, Some(_)) => err!(EcolLoteError::VariacaoInvalida),
            (_, None) => err!(EcolLoteError::VariacaoObrigatoria),
            (_, Some(v)) => {
                require!(
                    v.material == self.codigo && v.indice >= 1 && v.indice <= self.qtd_variacoes,
                    EcolLoteError::VariacaoInvalida
                );
                require!(v.ativa, EcolLoteError::VariacaoInativa);
                Ok(v.indice)
            }
        }
    }

    /// Garrafas só em material que conta garrafas.
    pub fn exigir_garrafas(&self, qtd_garrafas: u32) -> Result<()> {
        require!(
            qtd_garrafas == 0 || self.conta_garrafas,
            EcolLoteError::MaterialSemGarrafas
        );
        Ok(())
    }
}

impl MaterialVariacao {
    pub fn pda(codigo: u16, indice: u8) -> Pubkey {
        Pubkey::find_program_address(
            &[SEED_VARIACAO, &codigo.to_le_bytes(), &[indice]],
            &crate::ID,
        )
        .0
    }
}
