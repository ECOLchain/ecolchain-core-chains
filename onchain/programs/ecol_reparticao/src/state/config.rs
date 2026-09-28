use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_BASE, ECOLCHAIN_MIN_BPS},
    error::EcolReparticaoError,
};

/// Percentuais (basis points) sobre a receita da venda do crédito.
/// Coletor, cooperativa e transportador são fixos; a indústria negocia com a EcolChain dentro
/// da faixa, e a EcolChain fica com o restante (incluindo o resíduo de arredondamento).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub struct BpsReparticao {
    pub coletor: u16,
    pub cooperativa: u16,
    pub transportador: u16,
    pub industria_min: u16,
    pub industria_max: u16,
}

impl BpsReparticao {
    pub fn fixos(&self) -> Result<u16> {
        self.coletor
            .checked_add(self.cooperativa)
            .and_then(|v| v.checked_add(self.transportador))
            .ok_or_else(|| error!(EcolReparticaoError::PercentuaisInvalidos))
    }

    /// Faixas coerentes: EcolChain nunca abaixo de 1%, mesmo com a indústria no máximo.
    pub fn validar(&self) -> Result<()> {
        require!(
            self.coletor > 0 && self.cooperativa > 0 && self.transportador > 0,
            EcolReparticaoError::PercentuaisInvalidos
        );
        require!(
            self.industria_min > 0 && self.industria_min <= self.industria_max,
            EcolReparticaoError::PercentuaisInvalidos
        );
        let teto = u32::from(self.fixos()?)
            .checked_add(u32::from(self.industria_max))
            .and_then(|v| v.checked_add(u32::from(ECOLCHAIN_MIN_BPS)))
            .ok_or(EcolReparticaoError::PercentuaisInvalidos)?;
        require!(
            teto <= BPS_BASE as u32,
            EcolReparticaoError::PercentuaisInvalidos
        );
        Ok(())
    }

    pub fn exigir_faixa_industria(&self, bps_industria: u16) -> Result<()> {
        require!(
            (self.industria_min..=self.industria_max).contains(&bps_industria),
            EcolReparticaoError::IndustriaForaDaFaixa
        );
        Ok(())
    }
}

/// Configuração (PDA `["rep_config"]`). Mudanças de percentual passam pelo admin (Squads com
/// timelock) e não afetam distribuições já criadas, que guardam uma cópia dos percentuais.
#[account]
#[derive(InitSpace)]
pub struct ReparticaoConfig {
    pub admin: Pubkey,
    pub admin_pendente: Pubkey,
    /// Backend: publica a árvore de direitos de cada crédito vendido.
    pub operador: Pubkey,
    /// Plataforma de cupons: co-assina cada resgate.
    pub zupy: Pubkey,
    /// Intermediador de pagamento: atesta o repasse da receita à Zupy e à EcolChain.
    pub intermediador: Pubkey,
    pub bps: BpsReparticao,
    pub pausado: bool,
    pub bump: u8,
    pub _reserved: [u8; 64],
}

impl ReparticaoConfig {
    pub fn exigir_ativo(&self) -> Result<()> {
        require!(!self.pausado, EcolReparticaoError::ProgramaPausado);
        Ok(())
    }

    pub fn eh_chave_de_papel(&self, chave: &Pubkey) -> bool {
        [self.admin, self.operador, self.zupy, self.intermediador].contains(chave)
    }

    pub fn validar_chaves(&self) -> Result<()> {
        let chaves = [self.admin, self.operador, self.zupy, self.intermediador];
        for (i, c) in chaves.iter().enumerate() {
            require!(
                *c != Pubkey::default(),
                EcolReparticaoError::ChavesDeConfigInvalidas
            );
            require!(
                !chaves[..i].contains(c),
                EcolReparticaoError::ChavesDeConfigInvalidas
            );
        }
        Ok(())
    }
}
