use anchor_lang::prelude::*;
use ecol_lote::Material;

use crate::{constants::SERIAL_MAX_LEN, error::EcolCreditoError};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum PadraoRegistro {
    Verra,
    GoldStandard,
}

/// Máquina de estados do crédito.
///
/// ```text
/// EmFormacao → EmValidacao → Emitido → Vendido → Aposentado
///     ↘ Abandonado          ↘ Rejeitado
/// ```
/// Em `Rejeitado` e `Abandonado` os lotes podem ser liberados (voltam a `Reciclado`).
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum EstadoCredito {
    /// Recebendo lotes reciclados.
    EmFormacao,
    /// Lista de lotes fechada e submetida ao validador/registro.
    EmValidacao,
    /// Emitido no registro oficial, com número de série.
    Emitido,
    /// Vendido a um comprador (valor em centavos de R$, pago fora da blockchain).
    Vendido,
    /// Aposentado no registro em nome do comprador (terminal).
    Aposentado,
    /// Não validado (terminal): sem emissão e sem repartição.
    Rejeitado,
    /// Passou do prazo em formação sem ser submetido (terminal).
    Abandonado,
}

impl EstadoCredito {
    pub fn exigir_em_formacao(self) -> Result<()> {
        require!(
            self == EstadoCredito::EmFormacao,
            EcolCreditoError::TransicaoInvalida
        );
        Ok(())
    }

    pub fn abandonar(self, agora: i64, prazo_formacao: i64) -> Result<Self> {
        match self {
            EstadoCredito::EmFormacao => {
                require!(agora > prazo_formacao, EcolCreditoError::PrazoNaoExpirado);
                Ok(EstadoCredito::Abandonado)
            }
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }

    /// Estados em que os lotes agregados podem voltar a `Reciclado`.
    pub fn permite_liberar_lotes(self) -> bool {
        matches!(self, EstadoCredito::Rejeitado | EstadoCredito::Abandonado)
    }

    pub fn submeter(self) -> Result<Self> {
        match self {
            EstadoCredito::EmFormacao => Ok(EstadoCredito::EmValidacao),
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }

    pub fn emitir(self) -> Result<Self> {
        match self {
            EstadoCredito::EmValidacao => Ok(EstadoCredito::Emitido),
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }

    pub fn rejeitar(self) -> Result<Self> {
        match self {
            EstadoCredito::EmValidacao => Ok(EstadoCredito::Rejeitado),
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }

    pub fn vender(self) -> Result<Self> {
        match self {
            EstadoCredito::Emitido => Ok(EstadoCredito::Vendido),
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }

    pub fn aposentar(self) -> Result<Self> {
        match self {
            EstadoCredito::Vendido => Ok(EstadoCredito::Aposentado),
            _ => err!(EcolCreditoError::TransicaoInvalida),
        }
    }
}

/// Crédito de carbono (PDA `["credito", credito_id]`). Não é token: só registra a ligação
/// entre o crédito do registro oficial e os lotes físicos que o originaram.
#[account]
#[derive(InitSpace)]
pub struct Credito {
    pub credito_id: u64,
    pub padrao: PadraoRegistro,
    /// Material de todos os lotes agregados (ex.: crédito de plástico só com lotes de plástico).
    pub material: Material,
    pub vintage: u16,
    /// Hash do documento de metodologia/projeto submetido ao validador.
    pub metodologia_hash: [u8; 32],
    pub qtd_lotes: u32,
    /// Lotes já devolvidos a `Reciclado` após rejeição/abandono.
    pub qtd_lotes_liberados: u32,
    pub peso_total_g: u64,
    /// Hash encadeado dos lotes: `h = sha256(h || lote || peso_g)`, na ordem de agregação.
    /// Qualquer auditor reconstrói a partir dos eventos `LoteAgregado`.
    pub lotes_hash: [u8; 32],
    /// Toneladas de CO2e emitidas × 1000.
    pub tco2e_milli: u64,
    pub comprador: Pubkey,
    pub valor_venda_centavos: u64,
    /// Hash da referência do pagamento no intermediador.
    pub pagamento_ref_hash: [u8; 32],
    /// Limite para sair de `EmFormacao`.
    pub prazo_formacao: i64,
    /// Hash da prova de aposentadoria no registro (certificado / link).
    pub prova_aposentadoria_hash: [u8; 32],
    pub estado: EstadoCredito,
    pub seq: u64,
    pub criado_em: i64,
    pub atualizado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 64],
    /// Número de série no registro oficial (campo variável por último).
    #[max_len(SERIAL_MAX_LEN)]
    pub serial: String,
}

impl Credito {
    pub fn proximo_seq(&mut self) -> Result<u64> {
        self.seq = self.seq.checked_add(1).ok_or(EcolCreditoError::Overflow)?;
        Ok(self.seq)
    }

    pub fn transicionar(
        &mut self,
        novo: EstadoCredito,
        agora: i64,
    ) -> Result<(EstadoCredito, u64)> {
        let anterior = self.estado;
        self.estado = novo;
        self.atualizado_em = agora;
        Ok((anterior, self.proximo_seq()?))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use EstadoCredito::*;

    #[test]
    fn matriz_completa_de_transicoes() {
        type T = (&'static str, fn(EstadoCredito) -> Result<EstadoCredito>);
        let transicoes: [T; 6] = [
            ("abandonar", |e| e.abandonar(2, 1)),
            ("submeter", |e| e.submeter()),
            ("emitir", |e| e.emitir()),
            ("rejeitar", |e| e.rejeitar()),
            ("vender", |e| e.vender()),
            ("aposentar", |e| e.aposentar()),
        ];
        let permitida = |e: EstadoCredito, t: &str| {
            matches!(
                (e, t),
                (EmFormacao, "submeter")
                    | (EmValidacao, "emitir" | "rejeitar")
                    | (Emitido, "vender")
                    | (Vendido, "aposentar")
                    | (EmFormacao, "abandonar")
            )
        };
        for e in [
            EmFormacao,
            EmValidacao,
            Emitido,
            Vendido,
            Aposentado,
            Rejeitado,
            Abandonado,
        ] {
            for (nome, f) in transicoes {
                assert_eq!(f(e).is_ok(), permitida(e, nome), "{e:?} --{nome}-->");
            }
        }
    }

    #[test]
    fn abandono_so_depois_do_prazo() {
        assert!(EmFormacao.abandonar(100, 100).is_err());
        assert_eq!(EmFormacao.abandonar(101, 100).unwrap(), Abandonado);
        assert!(Rejeitado.permite_liberar_lotes() && Abandonado.permite_liberar_lotes());
        assert!(!EmFormacao.permite_liberar_lotes() && !Aposentado.permite_liberar_lotes());
    }
}
