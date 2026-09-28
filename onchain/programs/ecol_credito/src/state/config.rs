use anchor_lang::prelude::*;

use crate::error::EcolCreditoError;

/// Configuração do programa de créditos (PDA `["credito_config"]`).
#[account]
#[derive(InitSpace)]
pub struct CreditoConfig {
    /// Squads multisig.
    pub admin: Pubkey,
    /// Proposta de troca de admin, efetivada quando o novo admin assina.
    pub admin_pendente: Pubkey,
    /// Backend da EcolChain: monta créditos e registra vendas.
    pub operador: Pubkey,
    /// Chave que atesta o que aconteceu no registro oficial (Verra / Gold Standard):
    /// emissão com número de série, rejeição e aposentadoria.
    pub registrador: Pubkey,
    /// Intermediador de pagamento: atesta o recebimento do valor da venda do crédito.
    pub intermediador: Pubkey,
    pub pausado: bool,
    pub bump: u8,
    pub _reserved: [u8; 64],
}

impl CreditoConfig {
    pub fn eh_chave_de_papel(&self, chave: &Pubkey) -> bool {
        [
            self.admin,
            self.operador,
            self.registrador,
            self.intermediador,
        ]
        .contains(chave)
    }

    pub fn exigir_ativo(&self) -> Result<()> {
        require!(!self.pausado, EcolCreditoError::ProgramaPausado);
        Ok(())
    }

    /// Um papel por chave, nenhuma nula.
    pub fn validar_chaves(&self) -> Result<()> {
        let chaves = [
            self.admin,
            self.operador,
            self.registrador,
            self.intermediador,
        ];
        for (i, c) in chaves.iter().enumerate() {
            require!(
                *c != Pubkey::default(),
                EcolCreditoError::ChavesDeConfigInvalidas
            );
            require!(
                !chaves[..i].contains(c),
                EcolCreditoError::ChavesDeConfigInvalidas
            );
        }
        Ok(())
    }
}

/// Garante que um número de série do registro só é usado por um crédito
/// (PDA `["serial", padrão, sha256(serial)]`).
#[account]
#[derive(InitSpace)]
pub struct SerialRegistro {
    pub credito: Pubkey,
    pub bump: u8,
}
