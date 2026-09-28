use anchor_lang::prelude::*;
use ecol_lote::Papel;

use crate::{
    constants::BPS_BASE, error::EcolReparticaoError, merkle::papel_u8, state::BpsReparticao,
};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum EstadoDistribuicao {
    /// Árvore publicada; aguardando o intermediador repassar a receita.
    Pendente,
    /// Receita repassada (Zupy e EcolChain); resgates liberados.
    Financiada,
}

/// Direitos sobre a receita de um crédito vendido (PDA `["dist", credito]`).
#[account]
#[derive(InitSpace)]
pub struct Distribuicao {
    pub credito: Pubkey,
    pub receita_centavos: u64,
    /// Cópia dos percentuais vigentes na criação.
    pub bps: BpsReparticao,
    /// Percentual negociado da indústria para este crédito.
    pub bps_industria: u16,
    /// Teto por papel (coletor, cooperativa, transportador, indústria), calculado on-chain.
    pub total_por_papel: [u64; 4],
    pub resgatado_por_papel: [u64; 4],
    /// Parte da EcolChain (restante, inclui o resíduo de arredondamento), paga em R$.
    pub ecolchain_centavos: u64,
    pub merkle_root: [u8; 32],
    pub num_folhas: u32,
    pub qtd_resgates: u32,
    pub estado: EstadoDistribuicao,
    pub repasse_zupy_ref_hash: [u8; 32],
    pub repasse_ecolchain_ref_hash: [u8; 32],
    pub seq: u64,
    pub criado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 32],
    /// Um bit por folha: resgate já registrado (campo variável por último).
    #[max_len(0)]
    pub resgatados: Vec<u8>,
}

/// `floor(receita × bps / 10_000)`.
pub fn parcela(receita_centavos: u64, bps: u16) -> Result<u64> {
    let v = u128::from(receita_centavos)
        .checked_mul(u128::from(bps))
        .and_then(|v| v.checked_div(u128::from(BPS_BASE)))
        .ok_or(EcolReparticaoError::Overflow)?;
    u64::try_from(v).map_err(|_| error!(EcolReparticaoError::Overflow))
}

impl Distribuicao {
    pub fn bytes_bitmap(num_folhas: u32) -> usize {
        (num_folhas as usize).div_ceil(8)
    }

    /// Discriminator + campos fixos + bitmap (limitado por `MAX_FOLHAS`, então não satura na prática).
    pub fn espaco(num_folhas: u32) -> usize {
        8usize
            .saturating_add(Distribuicao::INIT_SPACE)
            .saturating_add(Distribuicao::bytes_bitmap(num_folhas))
    }

    /// Calcula os tetos por papel; a EcolChain fica com o restante.
    pub fn calcular(
        &mut self,
        receita_centavos: u64,
        bps: BpsReparticao,
        bps_industria: u16,
    ) -> Result<()> {
        bps.exigir_faixa_industria(bps_industria)?;
        let totais = [
            parcela(receita_centavos, bps.coletor)?,
            parcela(receita_centavos, bps.cooperativa)?,
            parcela(receita_centavos, bps.transportador)?,
            parcela(receita_centavos, bps_industria)?,
        ];
        let soma = totais
            .iter()
            .try_fold(0u64, |acc, v| acc.checked_add(*v))
            .ok_or(EcolReparticaoError::Overflow)?;
        self.ecolchain_centavos = receita_centavos
            .checked_sub(soma)
            .ok_or(EcolReparticaoError::Overflow)?;
        self.receita_centavos = receita_centavos;
        self.bps = bps;
        self.bps_industria = bps_industria;
        self.total_por_papel = totais;
        Ok(())
    }

    pub fn total_cupons(&self) -> Result<u64> {
        self.total_por_papel
            .iter()
            .try_fold(0u64, |acc, v| acc.checked_add(*v))
            .ok_or_else(|| error!(EcolReparticaoError::Overflow))
    }

    pub fn ja_resgatado(&self, indice: u32) -> bool {
        let (byte, bit) = ((indice / 8) as usize, indice % 8);
        self.resgatados
            .get(byte)
            .is_some_and(|b| b & (1 << bit) != 0)
    }

    /// Marca a folha e soma ao papel, sem deixar o papel passar do seu teto.
    pub fn registrar_resgate(
        &mut self,
        indice: u32,
        papel: Papel,
        valor_centavos: u64,
    ) -> Result<()> {
        require!(
            indice < self.num_folhas,
            EcolReparticaoError::IndiceInvalido
        );
        require!(!self.ja_resgatado(indice), EcolReparticaoError::JaResgatado);
        let p = usize::from(papel_u8(papel));
        let novo = self.resgatado_por_papel[p]
            .checked_add(valor_centavos)
            .ok_or(EcolReparticaoError::Overflow)?;
        require!(
            novo <= self.total_por_papel[p],
            EcolReparticaoError::TetoDoPapelExcedido
        );
        self.resgatado_por_papel[p] = novo;
        let (byte, bit) = ((indice / 8) as usize, indice % 8);
        let b = self
            .resgatados
            .get_mut(byte)
            .ok_or(EcolReparticaoError::IndiceInvalido)?;
        *b |= 1 << bit;
        self.qtd_resgates = self
            .qtd_resgates
            .checked_add(1)
            .ok_or(EcolReparticaoError::Overflow)?;
        Ok(())
    }

    pub fn proximo_seq(&mut self) -> Result<u64> {
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or(EcolReparticaoError::Overflow)?;
        Ok(self.seq)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn padrao() -> BpsReparticao {
        BpsReparticao {
            coletor: 500,
            cooperativa: 1_000,
            transportador: 500,
            industria_min: 6_500,
            industria_max: 7_900,
        }
    }

    fn vazia() -> Distribuicao {
        Distribuicao {
            credito: Pubkey::default(),
            receita_centavos: 0,
            bps: padrao(),
            bps_industria: 0,
            total_por_papel: [0; 4],
            resgatado_por_papel: [0; 4],
            ecolchain_centavos: 0,
            merkle_root: [0; 32],
            num_folhas: 10,
            qtd_resgates: 0,
            estado: EstadoDistribuicao::Financiada,
            repasse_zupy_ref_hash: [0; 32],
            repasse_ecolchain_ref_hash: [0; 32],
            seq: 0,
            criado_em: 0,
            bump: 0,
            _reserved: [0; 32],
            resgatados: vec![0; 2],
        }
    }

    #[test]
    fn exemplo_do_diagrama_us_50() {
        // US$ 50,00 = 5.000 centavos: 250 / 500 / 250 / 3.250 e EcolChain 750 (15%).
        let mut d = vazia();
        d.calcular(5_000, padrao(), 6_500).unwrap();
        assert_eq!(d.total_por_papel, [250, 500, 250, 3_250]);
        assert_eq!(d.ecolchain_centavos, 750);
        // Indústria no máximo (79%): EcolChain 1%.
        d.calcular(5_000, padrao(), 7_900).unwrap();
        assert_eq!((d.total_por_papel[3], d.ecolchain_centavos), (3_950, 50));
    }

    #[test]
    fn residuo_vai_para_a_ecolchain_e_soma_fecha() {
        let mut d = vazia();
        for receita in [1u64, 7, 99, 12_345, 1_000_003, u64::MAX / 10_000] {
            d.calcular(receita, padrao(), 7_000).unwrap();
            assert_eq!(d.total_cupons().unwrap() + d.ecolchain_centavos, receita);
        }
    }

    #[test]
    fn faixas() {
        let mut d = vazia();
        assert!(d.calcular(5_000, padrao(), 6_499).is_err());
        assert!(d.calcular(5_000, padrao(), 7_901).is_err());
        assert!(padrao().validar().is_ok());
        let mut b = padrao();
        b.industria_max = 8_000; // EcolChain ficaria com 0%
        assert!(b.validar().is_err());
        let mut b = padrao();
        b.industria_min = 8_000;
        assert!(b.validar().is_err());
    }

    #[test]
    fn resgate_respeita_bitmap_e_teto() {
        let mut d = vazia();
        d.calcular(5_000, padrao(), 6_500).unwrap();
        d.registrar_resgate(0, Papel::Coletor, 200).unwrap();
        assert!(
            d.registrar_resgate(0, Papel::Coletor, 1).is_err(),
            "folha já usada"
        );
        assert!(
            d.registrar_resgate(1, Papel::Coletor, 51).is_err(),
            "passaria do teto de 250"
        );
        d.registrar_resgate(1, Papel::Coletor, 50).unwrap();
        assert!(
            d.registrar_resgate(10, Papel::Industria, 1).is_err(),
            "fora do num_folhas"
        );
        assert!(d.ja_resgatado(0) && d.ja_resgatado(1) && !d.ja_resgatado(2));
    }
}
