use anchor_lang::prelude::*;

use crate::{constants::BPS_BASE, error::EcolLoteError};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum MotivoDisputa {
    DivergenciaPeso,
    ClassificacaoContestada,
    FalhaEntrega,
    PrazoExpirado,
    Outro,
}

/// Etapas em que um lote pode entrar em disputa; é para elas que o lote volta se o árbitro mandar prosseguir.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum EtapaDisputavel {
    Vendido,
    EmTransporte,
    Recebido,
}

impl EtapaDisputavel {
    pub fn estado(self) -> EstadoLote {
        match self {
            EtapaDisputavel::Vendido => EstadoLote::Vendido,
            EtapaDisputavel::EmTransporte => EstadoLote::EmTransporte,
            EtapaDisputavel::Recebido => EstadoLote::Recebido,
        }
    }
}

/// Máquina de estados do lote.
///
/// ```text
/// Criado → Anunciado → Vendido → EmTransporte → Recebido → Reciclado → Agregado
///          Anunciado ⇄ SemLance
///          Vendido | EmTransporte | Recebido → EmDisputa → (etapa de origem | Reembolsado)
/// ```
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum EstadoLote {
    /// Pesado e registrado pela cooperativa.
    Criado,
    /// À venda no marketplace até `prazo_leilao`.
    Anunciado { prazo_leilao: i64 },
    /// Leilão encerrado sem lance vencedor; pode ser reanunciado.
    SemLance,
    /// Lance vencedor aceito e escrow em R$ depositado (atestado pelo intermediador).
    Vendido,
    /// Retirado pelo transportador.
    EmTransporte,
    /// Recebido pela indústria dentro da tolerância de peso.
    Recebido,
    /// Escrow liberado à cooperativa; operação física encerrada.
    Reciclado,
    /// Incluído num crédito de carbono. Só volta a `Reciclado` se o crédito for rejeitado ou abandonado.
    Agregado,
    EmDisputa {
        motivo: MotivoDisputa,
        origem: EtapaDisputavel,
    },
    /// Escrow devolvido à indústria (terminal).
    Reembolsado,
}

impl EstadoLote {
    pub fn anunciar(self, agora: i64, prazo_leilao: i64) -> Result<Self> {
        require!(prazo_leilao > agora, EcolLoteError::PrazoInvalido);
        match self {
            EstadoLote::Criado | EstadoLote::SemLance => Ok(EstadoLote::Anunciado { prazo_leilao }),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    /// A cooperativa retira o anúncio a qualquer momento (reprecificar ou desistir).
    pub fn cancelar_anuncio(self) -> Result<Self> {
        match self {
            EstadoLote::Anunciado { .. } => Ok(EstadoLote::SemLance),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn encerrar_sem_lance(self, agora: i64) -> Result<Self> {
        match self {
            EstadoLote::Anunciado { prazo_leilao } => {
                require!(agora >= prazo_leilao, EcolLoteError::LeilaoEmAndamento);
                Ok(EstadoLote::SemLance)
            }
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    /// O leilão é apurado off-chain; a venda pode ser registrada a qualquer momento enquanto o lote está anunciado.
    pub fn vender(self) -> Result<Self> {
        match self {
            EstadoLote::Anunciado { .. } => Ok(EstadoLote::Vendido),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn iniciar_transporte(self) -> Result<Self> {
        match self {
            EstadoLote::Vendido => Ok(EstadoLote::EmTransporte),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn receber(self, peso_divergente: bool) -> Result<Self> {
        match self {
            EstadoLote::EmTransporte if peso_divergente => Ok(EstadoLote::EmDisputa {
                motivo: MotivoDisputa::DivergenciaPeso,
                origem: EtapaDisputavel::Recebido,
            }),
            EstadoLote::EmTransporte => Ok(EstadoLote::Recebido),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn liberar_escrow(self) -> Result<Self> {
        match self {
            EstadoLote::Recebido => Ok(EstadoLote::Reciclado),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn etapa_disputavel(self) -> Option<EtapaDisputavel> {
        match self {
            EstadoLote::Vendido => Some(EtapaDisputavel::Vendido),
            EstadoLote::EmTransporte => Some(EtapaDisputavel::EmTransporte),
            EstadoLote::Recebido => Some(EtapaDisputavel::Recebido),
            _ => None,
        }
    }

    pub fn abrir_disputa(self, motivo: MotivoDisputa) -> Result<Self> {
        let origem = self
            .etapa_disputavel()
            .ok_or(EcolLoteError::TransicaoInvalida)?;
        Ok(EstadoLote::EmDisputa { motivo, origem })
    }

    pub fn expirar_prazo(self, agora: i64, prazo_entrega: i64) -> Result<Self> {
        match self {
            EstadoLote::Vendido | EstadoLote::EmTransporte => {
                require!(agora > prazo_entrega, EcolLoteError::PrazoNaoExpirado);
                self.abrir_disputa(MotivoDisputa::PrazoExpirado)
            }
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn resolver_prosseguir(self) -> Result<Self> {
        match self {
            EstadoLote::EmDisputa { origem, .. } => Ok(origem.estado()),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    /// Árbitro resolve e liquida de uma vez (total ou parcial). Só depois da entrega.
    pub fn resolver_liberacao(self) -> Result<Self> {
        match self {
            EstadoLote::EmDisputa {
                origem: EtapaDisputavel::Recebido,
                ..
            } => Ok(EstadoLote::Reciclado),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn resolver_reembolso(self) -> Result<Self> {
        match self {
            EstadoLote::EmDisputa { .. } => Ok(EstadoLote::Reembolsado),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    /// Crédito rejeitado ou abandonado: o lote volta a poder integrar outro crédito.
    pub fn desagregar(self) -> Result<Self> {
        match self {
            EstadoLote::Agregado => Ok(EstadoLote::Reciclado),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }

    pub fn agregar(self) -> Result<Self> {
        match self {
            EstadoLote::Reciclado => Ok(EstadoLote::Agregado),
            _ => err!(EcolLoteError::TransicaoInvalida),
        }
    }
}

/// Estado e registro de um lote (PDA `["lote", cooperativa, lote_id]`).
#[account]
#[derive(InitSpace)]
pub struct Lote {
    pub cooperativa: Pubkey,
    pub lote_id: u64,
    /// Código do material cadastrado (conta `Material`), imutável depois da criação.
    pub material: u16,
    /// Peso vigente em gramas (da balança da cooperativa, ou reclassificado pelo árbitro).
    pub peso_g: u64,
    /// Peso informado pela indústria no recebimento.
    pub peso_recebido_g: u64,
    /// Entregas de coletores vinculadas ao lote (montagem). Os coletores estão nas entregas.
    pub qtd_entregas: u32,
    /// Soma dos pesos das entregas (o `peso_g` é a pesagem do lote montado).
    pub peso_entregas_g: u64,
    /// Hash encadeado das entregas: `h = sha256(h || entrega || peso_g)`, na ordem de vinculação.
    pub entregas_hash: [u8; 32],
    /// Hash das evidências off-chain (fotos, ticket da balança).
    pub evidencias_hash: [u8; 32],
    pub preco_minimo_centavos: u64,
    pub industria: Pubkey,
    pub transportador: Pubkey,
    /// Valor da venda em centavos de R$ (o dinheiro fica no escrow off-chain).
    pub valor_centavos: u64,
    /// Hash da referência do escrow no intermediador (depósito).
    pub escrow_ref_hash: [u8; 32],
    /// Raiz Merkle de todos os lances do leilão, para auditoria da apuração.
    pub lances_root: [u8; 32],
    /// Prazo para o recebimento, fixado na venda.
    pub prazo_entrega: i64,
    /// Tolerância de peso vigente na venda (mudanças posteriores do config não afetam o lote).
    pub tolerancia_peso_bps: u16,
    /// Quanto do escrow foi liberado à cooperativa (o resto, se houver, foi reembolsado).
    pub valor_liberado_centavos: u64,
    /// Etapa cuja disputa o árbitro já julgou com "prosseguir": as partes não a reabrem.
    pub etapa_julgada: Option<EtapaDisputavel>,
    /// Crédito de carbono ao qual o lote foi agregado.
    pub credito: Pubkey,
    /// Balança (PDA) que atestou a pesagem de origem.
    pub balanca_origem: Pubkey,
    /// Horário assinado pela balança na pesagem de origem.
    pub ts_pesagem: i64,
    /// Core asset (recibo) do lote.
    pub asset: Pubkey,
    /// Recebimento confirmado com pesagem assinada por balança da indústria.
    pub recebimento_atestado: bool,
    pub estado: EstadoLote,
    /// Número de sequência do último evento emitido para este lote (auditoria detecta lacunas).
    pub seq: u64,
    pub criado_em: i64,
    pub atualizado_em: i64,
    pub bump: u8,
    pub _reserved: [u8; 64],
}

impl Lote {
    pub fn proximo_seq(&mut self) -> Result<u64> {
        self.seq = self.seq.checked_add(1).ok_or(EcolLoteError::Overflow)?;
        Ok(self.seq)
    }

    /// Aplica a transição e devolve `(estado_anterior, seq)` para o evento.
    pub fn transicionar(&mut self, novo: EstadoLote, agora: i64) -> Result<(EstadoLote, u64)> {
        let anterior = self.estado;
        self.estado = novo;
        self.atualizado_em = agora;
        Ok((anterior, self.proximo_seq()?))
    }

    /// Diverge quando `|recebido - peso_g| / peso_g` passa de `tolerancia_bps`.
    pub fn peso_diverge(&self, peso_recebido_g: u64, tolerancia_bps: u16) -> Result<bool> {
        let diferenca = u128::from(self.peso_g.abs_diff(peso_recebido_g));
        let limite = u128::from(self.peso_g)
            .checked_mul(u128::from(tolerancia_bps))
            .ok_or(EcolLoteError::Overflow)?;
        let diferenca_bps = diferenca
            .checked_mul(u128::from(BPS_BASE))
            .ok_or(EcolLoteError::Overflow)?;
        Ok(diferenca_bps > limite)
    }

    pub fn eh_parte(&self, chave: &Pubkey) -> bool {
        *chave != Pubkey::default()
            && (*chave == self.cooperativa
                || *chave == self.industria
                || *chave == self.transportador)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const AGORA: i64 = 1_000;

    fn todos_os_estados() -> Vec<EstadoLote> {
        vec![
            EstadoLote::Criado,
            EstadoLote::Anunciado {
                prazo_leilao: AGORA,
            },
            EstadoLote::SemLance,
            EstadoLote::Vendido,
            EstadoLote::EmTransporte,
            EstadoLote::Recebido,
            EstadoLote::Reciclado,
            EstadoLote::Agregado,
            EstadoLote::EmDisputa {
                motivo: MotivoDisputa::Outro,
                origem: EtapaDisputavel::EmTransporte,
            },
            EstadoLote::EmDisputa {
                motivo: MotivoDisputa::Outro,
                origem: EtapaDisputavel::Recebido,
            },
            EstadoLote::Reembolsado,
        ]
    }

    type Transicao = (&'static str, fn(EstadoLote) -> Result<EstadoLote>);

    /// Todas as transições, com o relógio já satisfazendo os prazos.
    fn transicoes() -> Vec<Transicao> {
        vec![
            ("anunciar", |e| e.anunciar(AGORA, AGORA + 1)),
            ("encerrar_sem_lance", |e| e.encerrar_sem_lance(AGORA)),
            ("vender", |e| e.vender()),
            ("iniciar_transporte", |e| e.iniciar_transporte()),
            ("receber", |e| e.receber(false)),
            ("liberar_escrow", |e| e.liberar_escrow()),
            ("abrir_disputa", |e| e.abrir_disputa(MotivoDisputa::Outro)),
            ("expirar_prazo", |e| e.expirar_prazo(AGORA + 1, AGORA)),
            ("resolver_prosseguir", |e| e.resolver_prosseguir()),
            ("resolver_reembolso", |e| e.resolver_reembolso()),
            ("resolver_liberacao", |e| e.resolver_liberacao()),
            ("cancelar_anuncio", |e| e.cancelar_anuncio()),
            ("agregar", |e| e.agregar()),
            ("desagregar", |e| e.desagregar()),
        ]
    }

    /// Única fonte de verdade das transições permitidas; qualquer par fora daqui deve falhar.
    fn permitida(estado: &EstadoLote, transicao: &str) -> bool {
        use EstadoLote::*;
        matches!(
            (estado, transicao),
            (Criado | SemLance, "anunciar")
                | (
                    Anunciado { .. },
                    "encerrar_sem_lance" | "vender" | "cancelar_anuncio"
                )
                | (
                    Vendido,
                    "iniciar_transporte" | "abrir_disputa" | "expirar_prazo"
                )
                | (EmTransporte, "receber" | "abrir_disputa" | "expirar_prazo")
                | (Recebido, "liberar_escrow" | "abrir_disputa")
                | (Reciclado, "agregar")
                | (Agregado, "desagregar")
                | (
                    EmDisputa { .. },
                    "resolver_prosseguir" | "resolver_reembolso"
                )
                | (
                    EmDisputa {
                        origem: EtapaDisputavel::Recebido,
                        ..
                    },
                    "resolver_liberacao"
                )
        )
    }

    #[test]
    fn matriz_completa_de_transicoes() {
        for estado in todos_os_estados() {
            for (nome, f) in transicoes() {
                let resultado = f(estado);
                assert_eq!(
                    resultado.is_ok(),
                    permitida(&estado, nome),
                    "{estado:?} --{nome}--> {resultado:?}"
                );
            }
        }
    }

    #[test]
    fn terminais_nao_saem() {
        assert!(transicoes()
            .iter()
            .all(|(_, f)| f(EstadoLote::Reembolsado).is_err()));
        // Agregado só sai por desagregação (crédito rejeitado/abandonado).
        let saidas: Vec<_> = transicoes()
            .iter()
            .filter(|(_, f)| f(EstadoLote::Agregado).is_ok())
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(saidas, vec!["desagregar"]);
    }

    #[test]
    fn prosseguir_volta_a_origem() {
        for origem in [
            EtapaDisputavel::Vendido,
            EtapaDisputavel::EmTransporte,
            EtapaDisputavel::Recebido,
        ] {
            let e = EstadoLote::EmDisputa {
                motivo: MotivoDisputa::Outro,
                origem,
            };
            assert_eq!(e.resolver_prosseguir().unwrap(), origem.estado());
        }
    }

    #[test]
    fn divergencia_de_peso_na_fronteira() {
        let lote = Lote {
            cooperativa: Pubkey::default(),
            lote_id: 0,
            material: 1,
            peso_g: 1_000_000,
            peso_recebido_g: 0,
            qtd_entregas: 0,
            peso_entregas_g: 0,
            entregas_hash: [0; 32],
            evidencias_hash: [0; 32],
            preco_minimo_centavos: 0,
            industria: Pubkey::default(),
            transportador: Pubkey::default(),
            valor_centavos: 0,
            escrow_ref_hash: [0; 32],
            lances_root: [0; 32],
            prazo_entrega: 0,
            tolerancia_peso_bps: 0,
            valor_liberado_centavos: 0,
            etapa_julgada: None,
            credito: Pubkey::default(),
            balanca_origem: Pubkey::default(),
            ts_pesagem: 0,
            asset: Pubkey::default(),
            recebimento_atestado: false,
            estado: EstadoLote::EmTransporte,
            seq: 0,
            criado_em: 0,
            atualizado_em: 0,
            bump: 0,
            _reserved: [0; 64],
        };
        // Tolerância de 2% = 20 kg: exatamente no limite não diverge; 1 g além diverge (para mais ou para menos).
        assert!(!lote.peso_diverge(980_000, 200).unwrap());
        assert!(!lote.peso_diverge(1_020_000, 200).unwrap());
        assert!(lote.peso_diverge(979_999, 200).unwrap());
        assert!(lote.peso_diverge(1_020_001, 200).unwrap());
        assert!(!lote.peso_diverge(1_000_000, 0).unwrap());
        assert!(lote.peso_diverge(999_999, 0).unwrap());
    }
}

#[cfg(test)]
mod propriedades {
    use super::*;
    use proptest::prelude::*;

    fn aplicar(e: EstadoLote, op: u8, agora: i64) -> Result<EstadoLote> {
        match op % 13 {
            0 => e.anunciar(agora, agora + 10),
            1 => e.encerrar_sem_lance(agora),
            2 => e.vender(),
            3 => e.iniciar_transporte(),
            4 => e.receber(agora % 2 == 0),
            5 => e.liberar_escrow(),
            6 => e.abrir_disputa(MotivoDisputa::Outro),
            7 => e.expirar_prazo(agora, 0),
            8 => e.resolver_prosseguir(),
            9 => e.resolver_reembolso(),
            10 => e.resolver_liberacao(),
            11 => e.agregar(),
            _ => e.cancelar_anuncio(),
        }
    }

    fn lote_com_peso(peso_g: u64) -> Lote {
        Lote {
            cooperativa: Pubkey::default(),
            lote_id: 0,
            material: 1,
            peso_g,
            peso_recebido_g: 0,
            qtd_entregas: 0,
            peso_entregas_g: 0,
            entregas_hash: [0; 32],
            evidencias_hash: [0; 32],
            preco_minimo_centavos: 0,
            industria: Pubkey::default(),
            transportador: Pubkey::default(),
            valor_centavos: 0,
            escrow_ref_hash: [0; 32],
            lances_root: [0; 32],
            prazo_entrega: 0,
            tolerancia_peso_bps: 0,
            valor_liberado_centavos: 0,
            etapa_julgada: None,
            credito: Pubkey::default(),
            balanca_origem: Pubkey::default(),
            ts_pesagem: 0,
            asset: Pubkey::default(),
            recebimento_atestado: false,
            estado: EstadoLote::Criado,
            seq: 0,
            criado_em: 0,
            atualizado_em: 0,
            bump: 0,
            _reserved: [0; 64],
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(5_000))]

        /// Sequências aleatórias de operações: Reembolsado é absorvente; nenhum caminho volta a Criado;
        /// só se chega a Agregado passando por Reciclado; transições rejeitadas não mudam o estado.
        #[test]
        fn sequencias_aleatorias_respeitam_o_grafo(ops in prop::collection::vec((any::<u8>(), 0i64..100), 1..60)) {
            let mut e = EstadoLote::Criado;
            let mut passou_por_reciclado = false;
            for (op, agora) in ops {
                let antes = e;
                match aplicar(e, op, agora) {
                    Ok(novo) => {
                        prop_assert_ne!(antes, EstadoLote::Reembolsado);
                        prop_assert_ne!(novo, EstadoLote::Criado);
                        if novo == EstadoLote::Agregado {
                            prop_assert_eq!(antes, EstadoLote::Reciclado);
                        }
                        e = novo;
                    }
                    Err(_) => prop_assert_eq!(e, antes),
                }
                passou_por_reciclado |= e == EstadoLote::Reciclado;
                if e == EstadoLote::Agregado {
                    prop_assert!(passou_por_reciclado);
                }
            }
        }

        /// Tolerância: simétrica (para mais ou para menos) e monotônica no desvio.
        #[test]
        fn tolerancia_simetrica_e_monotonica(peso in 1u64..u64::MAX / 2, desvio in 0u64..1_000_000, tol in 0u16..=2_000) {
            let l = lote_com_peso(peso);
            let desvio = desvio.min(peso);
            let a = l.peso_diverge(peso + desvio, tol).unwrap();
            let b = l.peso_diverge(peso - desvio, tol).unwrap();
            prop_assert_eq!(a, b);
            if a {
                prop_assert!(l.peso_diverge(peso + desvio + 1, tol).unwrap());
            }
        }
    }
}
