//! Árvore Merkle dos direitos. Folha e nó interno têm prefixos distintos (evita que um nó
//! interno seja apresentado como folha) e os pares são ordenados (prova sem bits de direção).
//!
//! O mesmo código é usado pelo gerador off-chain da árvore, então o auditor reconstrói a raiz
//! a partir dos lotes e dos percentuais publicados.

use anchor_lang::prelude::*;
use ecol_lote::Papel;
use solana_sha256_hasher::hashv;

pub fn papel_u8(papel: Papel) -> u8 {
    match papel {
        Papel::Coletor => 0,
        Papel::Cooperativa => 1,
        Papel::Transportador => 2,
        Papel::Industria => 3,
    }
}

pub fn folha(indice: u32, beneficiario: &Pubkey, papel: Papel, valor_centavos: u64) -> [u8; 32] {
    hashv(&[
        &[0u8],
        &indice.to_le_bytes(),
        beneficiario.as_ref(),
        &[papel_u8(papel)],
        &valor_centavos.to_le_bytes(),
    ])
    .to_bytes()
}

pub fn no(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let (menor, maior) = if a <= b { (a, b) } else { (b, a) };
    hashv(&[&[1u8], menor, maior]).to_bytes()
}

pub fn verificar(prova: &[[u8; 32]], folha: [u8; 32], raiz: &[u8; 32]) -> bool {
    prova.iter().fold(folha, |acc, irmao| no(&acc, irmao)) == *raiz
}

/// Raiz de uma lista de folhas (nível ímpar: o último sobe sem par). Usado off-chain e em testes.
pub fn raiz(folhas: &[[u8; 32]]) -> [u8; 32] {
    let mut nivel = folhas.to_vec();
    while nivel.len() > 1 {
        nivel = nivel
            .chunks(2)
            .map(|p| if p.len() == 2 { no(&p[0], &p[1]) } else { p[0] })
            .collect();
    }
    nivel.first().copied().unwrap_or([0; 32])
}

/// Prova da folha `indice`. Usado off-chain e em testes.
pub fn prova(folhas: &[[u8; 32]], mut indice: usize) -> Vec<[u8; 32]> {
    let mut nivel = folhas.to_vec();
    let mut caminho = vec![];
    while nivel.len() > 1 {
        let irmao = indice ^ 1;
        if let Some(h) = nivel.get(irmao) {
            caminho.push(*h);
        }
        nivel = nivel
            .chunks(2)
            .map(|p| if p.len() == 2 { no(&p[0], &p[1]) } else { p[0] })
            .collect();
        indice /= 2;
    }
    caminho
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn toda_folha_prova_contra_a_raiz() {
        for n in 1..=9usize {
            let folhas: Vec<[u8; 32]> = (0..n)
                .map(|i| folha(i as u32, &Pubkey::new_unique(), Papel::Coletor, i as u64))
                .collect();
            let r = raiz(&folhas);
            for i in 0..n {
                assert!(verificar(&prova(&folhas, i), folhas[i], &r), "n={n} i={i}");
            }
            // Um nó interno verifica com a prova encurtada (segunda pré-imagem). Por isso o
            // programa nunca aceita hash de folha vindo de fora: ele calcula a folha a partir de
            // (índice, beneficiário, papel, valor) com prefixo 0x00, que nunca coincide com um nó (0x01).
            if n >= 2 {
                let interno = no(&folhas[0], &folhas[1]);
                assert!(verificar(&prova(&folhas, 0)[1..], interno, &r));
                assert_ne!(folha(0, &Pubkey::default(), Papel::Coletor, 0), interno);
            }
        }
    }

    #[test]
    fn valor_alterado_nao_prova() {
        let b = Pubkey::new_unique();
        let folhas = vec![
            folha(0, &b, Papel::Coletor, 100),
            folha(1, &b, Papel::Industria, 900),
        ];
        let r = raiz(&folhas);
        let p = prova(&folhas, 0);
        assert!(!verificar(&p, folha(0, &b, Papel::Coletor, 101), &r));
        assert!(!verificar(&p, folha(0, &b, Papel::Cooperativa, 100), &r));
        assert!(!verificar(&p, folha(1, &b, Papel::Coletor, 100), &r));
    }
}

#[cfg(test)]
mod vetor_compartilhado {
    use super::*;

    /// Mesmo vetor em `clients/test/arvore.test.ts`: garante que o gerador TS e o programa
    /// calculam a mesma folha.
    #[test]
    fn folha_fixa() {
        let b = Pubkey::new_from_array([7; 32]);
        let h = folha(3, &b, Papel::Industria, 3_250);
        let hex: String = h.iter().map(|x| format!("{x:02x}")).collect();
        assert_eq!(hex, VETOR);
    }

    const VETOR: &str = "0bc16b3d8ce1484b7b16cabbad84e58357203e433e806442ad12b59549280d3c";
}

#[cfg(test)]
mod propriedades {
    use super::*;
    use proptest::prelude::*;

    fn papel(i: u8) -> Papel {
        [
            Papel::Coletor,
            Papel::Cooperativa,
            Papel::Transportador,
            Papel::Industria,
        ][usize::from(i % 4)]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Toda folha prova contra a raiz; alterar valor, papel, índice ou beneficiário quebra a prova.
        #[test]
        fn provas_validas_e_adulteracoes(
            dados in prop::collection::vec((any::<[u8; 32]>(), any::<u8>(), any::<u64>()), 1..64),
            alvo in any::<prop::sample::Index>(),
            delta in 1u64..u64::MAX,
        ) {
            let folhas: Vec<[u8; 32]> = dados.iter().enumerate()
                .map(|(i, (b, p, v))| folha(i as u32, &Pubkey::new_from_array(*b), papel(*p), *v))
                .collect();
            let r = raiz(&folhas);
            let i = alvo.index(folhas.len());
            let p = prova(&folhas, i);
            prop_assert!(verificar(&p, folhas[i], &r));

            let (b, pa, v) = dados[i];
            let beneficiario = Pubkey::new_from_array(b);
            prop_assert!(!verificar(&p, folha(i as u32, &beneficiario, papel(pa), v.wrapping_add(delta)), &r));
            prop_assert!(!verificar(&p, folha(i as u32, &beneficiario, papel(pa.wrapping_add(1)), v), &r));
            prop_assert!(!verificar(&p, folha((i as u32).wrapping_add(1), &beneficiario, papel(pa), v), &r));
            let mut outro = b;
            outro[0] ^= 1;
            prop_assert!(!verificar(&p, folha(i as u32, &Pubkey::new_from_array(outro), papel(pa), v), &r));
        }
    }
}
