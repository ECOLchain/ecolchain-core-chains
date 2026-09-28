//! `ecol_reparticao` com os três programas: lote → crédito vendido → direitos → resgates.
#[path = "../../ecol_credito/tests/cenario/mod.rs"]
mod cenario;
#[path = "../../ecol_lote/tests/common/mod.rs"]
mod common;

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    cenario::*,
    common::*,
    ecol_lote::Papel,
    ecol_reparticao::{
        accounts as ctas, error::EcolReparticaoError as R, instruction as ixs, merkle,
        BpsReparticao, Distribuicao, EstadoDistribuicao, ParamsReparticao, SEED_CONFIG,
        SEED_DISTRIBUICAO,
    },
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction::InstructionError,
    solana_transaction_error::TransactionError,
};

fn rpda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ecol_reparticao::id()).0
}
fn config_rep() -> Pubkey {
    rpda(&[SEED_CONFIG])
}
fn dist_pda(credito: &Pubkey) -> Pubkey {
    rpda(&[SEED_DISTRIBUICAO, credito.as_ref()])
}
fn event_authority_rep() -> Pubkey {
    rpda(&[b"__event_authority"])
}

fn bps_padrao() -> BpsReparticao {
    BpsReparticao {
        coletor: 500,
        cooperativa: 1_000,
        transportador: 500,
        industria_min: 6_500,
        industria_max: 7_900,
    }
}

#[track_caller]
fn assert_erro_rep(res: Result<TransactionMetadata, TransactionError>, e: R) {
    let esperado: u32 = e.into();
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, esperado)
        }
        outro => panic!("esperava {esperado}, veio {outro:?}"),
    }
}

/// Um direito (folha) da árvore.
struct Direito {
    beneficiario: Keypair,
    papel: Papel,
    valor: u64,
}

struct Rep {
    c: Cenario,
    zupy: Keypair,
    coletor: Keypair,
}

impl Rep {
    fn novo() -> Self {
        let mut c = Cenario::novo();
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/ecol_reparticao.so"
        ));
        c.amb.svm.add_program(ecol_reparticao::id(), bytes).unwrap();
        let pd = Pubkey::find_program_address(
            &[ecol_reparticao::id().as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        )
        .0;
        let mut conta = c.amb.svm.get_account(&pd).unwrap();
        conta.data[12] = 1;
        conta.data[13..45].copy_from_slice(c.amb.upgrade_authority.pubkey().as_ref());
        c.amb.svm.set_account(pd, conta).unwrap();

        let coletor = Keypair::new();
        c.amb.registrar(coletor.pubkey(), Papel::Coletor).unwrap();
        let mut r = Rep {
            c,
            zupy: Keypair::new(),
            coletor,
        };
        let ua = r.c.amb.upgrade_authority.insecure_clone();
        r.init(&ua).unwrap();
        r
    }

    fn ix(&self, data: impl InstructionData, contas: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(
            ecol_reparticao::id(),
            &data.data(),
            contas.to_account_metas(None),
        )
    }

    fn params(&self) -> ParamsReparticao {
        ParamsReparticao {
            operador: self.c.amb.operador.pubkey(),
            zupy: self.zupy.pubkey(),
            intermediador: self.c.amb.intermediador.pubkey(),
            bps: bps_padrao(),
        }
    }

    fn init(&mut self, ua: &Keypair) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitConfig {
                admin: self.c.amb.admin.pubkey(),
                params: self.params(),
            },
            ctas::AdminInitConfig {
                payer: self.c.amb.patrocinador.pubkey(),
                upgrade_authority: ua.pubkey(),
                config: config_rep(),
                este_programa: ecol_reparticao::id(),
                program_data: Pubkey::find_program_address(
                    &[ecol_reparticao::id().as_ref()],
                    &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
                )
                .0,
                system_program: system_program::ID,
                event_authority: event_authority_rep(),
                program: ecol_reparticao::id(),
            },
        );
        self.c.amb.enviar(ix, &[ua])
    }

    fn update(
        &mut self,
        params: ParamsReparticao,
    ) -> Result<TransactionMetadata, TransactionError> {
        let admin = self.c.amb.admin.insecure_clone();
        let ix = self.ix(
            ixs::AdminUpdateConfig { params },
            ctas::AdminUpdateConfig {
                admin: admin.pubkey(),
                config: config_rep(),
                event_authority: event_authority_rep(),
                program: ecol_reparticao::id(),
            },
        );
        self.c.amb.enviar(ix, &[&admin])
    }

    /// Crédito `id` com um lote, emitido e vendido por 5.000 centavos (US$ 50 do diagrama).
    fn credito_vendido(&mut self, id: u64) -> Pubkey {
        let lote = self.c.lote_reciclado(id);
        self.c.criar_credito(id);
        self.c.add_lotes(id, &[lote]).unwrap();
        self.c.submeter(id).unwrap();
        self.c.emitir(id, &format!("VCS-{id}")).unwrap();
        self.c.vender(id).unwrap();
        credito_pda(id)
    }

    /// Direitos "corretos" para 5.000 centavos com indústria a 65%.
    fn direitos_padrao(&self) -> Vec<Direito> {
        vec![
            Direito {
                beneficiario: self.coletor.insecure_clone(),
                papel: Papel::Coletor,
                valor: 250,
            },
            Direito {
                beneficiario: self.c.amb.cooperativa.insecure_clone(),
                papel: Papel::Cooperativa,
                valor: 500,
            },
            Direito {
                beneficiario: self.c.amb.transportador.insecure_clone(),
                papel: Papel::Transportador,
                valor: 250,
            },
            Direito {
                beneficiario: self.c.amb.industria.insecure_clone(),
                papel: Papel::Industria,
                valor: 3_250,
            },
        ]
    }

    fn folhas(direitos: &[Direito]) -> Vec<[u8; 32]> {
        direitos
            .iter()
            .enumerate()
            .map(|(i, d)| merkle::folha(i as u32, &d.beneficiario.pubkey(), d.papel, d.valor))
            .collect()
    }

    fn criar_dist(
        &mut self,
        credito: Pubkey,
        bps_industria: u16,
        raiz: [u8; 32],
        num_folhas: u32,
    ) -> Result<TransactionMetadata, TransactionError> {
        let op = self.c.amb.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorCreateDistribuicao {
                bps_industria,
                merkle_root: raiz,
                num_folhas,
            },
            ctas::OperadorCreateDistribuicao {
                payer: self.c.amb.patrocinador.pubkey(),
                operador: op.pubkey(),
                config: config_rep(),
                credito,
                distribuicao: dist_pda(&credito),
                system_program: system_program::ID,
                event_authority: event_authority_rep(),
                program: ecol_reparticao::id(),
            },
        );
        self.c.amb.enviar(ix, &[&op])
    }

    fn repasse_por(
        &mut self,
        inter: &Keypair,
        credito: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IntermediadorRecordRepasse {
                zupy_ref_hash: h("pix-zupy"),
                ecolchain_ref_hash: h("pix-ecolchain"),
            },
            ctas::IntermediadorRecordRepasse {
                intermediador: inter.pubkey(),
                config: config_rep(),
                distribuicao: dist_pda(&credito),
                event_authority: event_authority_rep(),
                program: ecol_reparticao::id(),
            },
        );
        self.c.amb.enviar(ix, &[inter])
    }

    fn repasse(&mut self, credito: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let inter = self.c.amb.intermediador.insecure_clone();
        self.repasse_por(&inter, credito)
    }

    #[allow(clippy::too_many_arguments)]
    fn resgatar_como(
        &mut self,
        zupy: &Keypair,
        assinante: &Keypair,
        credito: Pubkey,
        indice: u32,
        papel: Papel,
        valor: u64,
        prova: Vec<[u8; 32]>,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ZupyRecordResgate {
                indice,
                papel,
                valor_centavos: valor,
                prova,
                recibo_hash: h("cupom-123"),
            },
            ctas::ZupyRecordResgate {
                zupy: zupy.pubkey(),
                beneficiario: assinante.pubkey(),
                beneficiario_part: participante_pda(&assinante.pubkey()),
                config: config_rep(),
                distribuicao: dist_pda(&credito),
                event_authority: event_authority_rep(),
                program: ecol_reparticao::id(),
            },
        );
        self.c.amb.enviar(ix, &[zupy, assinante])
    }

    /// Resgate honesto da folha `i` da lista.
    fn resgatar(
        &mut self,
        credito: Pubkey,
        direitos: &[Direito],
        i: usize,
    ) -> Result<TransactionMetadata, TransactionError> {
        let folhas = Self::folhas(direitos);
        let (b, papel, valor) = (
            direitos[i].beneficiario.insecure_clone(),
            direitos[i].papel,
            direitos[i].valor,
        );
        let zupy = self.zupy.insecure_clone();
        self.resgatar_como(
            &zupy,
            &b,
            credito,
            i as u32,
            papel,
            valor,
            merkle::prova(&folhas, i),
        )
    }

    fn dist(&self, credito: Pubkey) -> Distribuicao {
        let conta = self.c.amb.svm.get_account(&dist_pda(&credito)).unwrap();
        Distribuicao::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Distribuição correta criada e financiada.
    fn financiada(&mut self, id: u64) -> (Pubkey, Vec<Direito>) {
        let credito = self.credito_vendido(id);
        let direitos = self.direitos_padrao();
        let raiz = merkle::raiz(&Self::folhas(&direitos));
        self.criar_dist(credito, 6_500, raiz, direitos.len() as u32)
            .unwrap();
        self.repasse(credito).unwrap();
        (credito, direitos)
    }
}

#[test]
fn reparticao_completa_do_exemplo_do_diagrama() {
    let mut r = Rep::novo();
    let credito = r.credito_vendido(1);
    let direitos = r.direitos_padrao();
    let raiz = merkle::raiz(&Rep::folhas(&direitos));
    r.criar_dist(credito, 6_500, raiz, 4).unwrap();

    let d = r.dist(credito);
    assert_eq!(d.receita_centavos, 5_000);
    assert_eq!(d.total_por_papel, [250, 500, 250, 3_250]);
    assert_eq!(d.ecolchain_centavos, 750);
    assert_eq!(d.estado, EstadoDistribuicao::Pendente);

    // Sem repasse do dinheiro, nada é resgatado.
    assert_erro_rep(r.resgatar(credito, &direitos, 0), R::EstadoInvalido);
    r.repasse(credito).unwrap();
    assert_erro_rep(r.repasse(credito), R::EstadoInvalido);

    for i in 0..4 {
        r.resgatar(credito, &direitos, i).unwrap();
    }
    let d = r.dist(credito);
    assert_eq!(d.resgatado_por_papel, d.total_por_papel);
    assert_eq!((d.qtd_resgates, d.seq), (4, 6));
}

#[test]
fn cada_direito_resgata_uma_vez() {
    let mut r = Rep::novo();
    let (credito, direitos) = r.financiada(1);
    r.resgatar(credito, &direitos, 0).unwrap();
    assert_erro_rep(r.resgatar(credito, &direitos, 0), R::JaResgatado);
}

#[test]
fn prova_amarra_valor_papel_e_beneficiario() {
    let mut r = Rep::novo();
    let (credito, direitos) = r.financiada(1);
    let folhas = Rep::folhas(&direitos);
    let zupy = r.zupy.insecure_clone();
    let coletor = r.coletor.insecure_clone();
    let p0 = merkle::prova(&folhas, 0);
    // Valor maior que o da folha.
    assert_erro_rep(
        r.resgatar_como(&zupy, &coletor, credito, 0, Papel::Coletor, 251, p0.clone()),
        R::ProvaInvalida,
    );
    // Outro coletor cadastrado tentando usar a folha do primeiro.
    let intruso = Keypair::new();
    r.c.amb.registrar(intruso.pubkey(), Papel::Coletor).unwrap();
    assert_erro_rep(
        r.resgatar_como(&zupy, &intruso, credito, 0, Papel::Coletor, 250, p0.clone()),
        R::ProvaInvalida,
    );
    // Cooperativa declarando papel de coletor: o cadastro dela não bate.
    let coop = r.c.amb.cooperativa.insecure_clone();
    assert_erro_rep(
        r.resgatar_como(
            &zupy,
            &coop,
            credito,
            1,
            Papel::Coletor,
            500,
            merkle::prova(&folhas, 1),
        ),
        R::PapelIncorreto,
    );
    // Carteira sem cadastro no ecol_lote.
    let anonimo = Keypair::new();
    assert!(r
        .resgatar_como(&zupy, &anonimo, credito, 0, Papel::Coletor, 250, p0)
        .is_err());
}

#[test]
fn arvore_gananciosa_nao_passa_do_teto_do_papel() {
    // O operador publica uma árvore que dá ao coletor 1.000 (teto calculado: 250).
    let mut r = Rep::novo();
    let credito = r.credito_vendido(1);
    let mut direitos = r.direitos_padrao();
    direitos[0].valor = 1_000;
    let raiz = merkle::raiz(&Rep::folhas(&direitos));
    r.criar_dist(credito, 6_500, raiz, 4).unwrap();
    r.repasse(credito).unwrap();
    assert_erro_rep(r.resgatar(credito, &direitos, 0), R::TetoDoPapelExcedido);
    // Os demais papéis seguem normais.
    r.resgatar(credito, &direitos, 1).unwrap();
}

#[test]
fn so_zupy_registra_resgate_e_so_intermediador_repassa() {
    let mut r = Rep::novo();
    let credito = r.credito_vendido(1);
    let direitos = r.direitos_padrao();
    let raiz = merkle::raiz(&Rep::folhas(&direitos));
    r.criar_dist(credito, 6_500, raiz, 4).unwrap();
    assert_erro_rep(
        r.repasse_por(&Keypair::new(), credito),
        R::NaoEhIntermediador,
    );
    r.repasse(credito).unwrap();

    let folhas = Rep::folhas(&direitos);
    let coletor = r.coletor.insecure_clone();
    assert_erro_rep(
        r.resgatar_como(
            &Keypair::new(),
            &coletor,
            credito,
            0,
            Papel::Coletor,
            250,
            merkle::prova(&folhas, 0),
        ),
        R::NaoEhZupy,
    );
}

#[test]
fn distribuicao_so_para_credito_vendido_e_uma_vez() {
    let mut r = Rep::novo();
    let lote = r.c.lote_reciclado(1);
    r.c.criar_credito(1);
    r.c.add_lotes(1, &[lote]).unwrap();
    r.c.submeter(1).unwrap();
    r.c.emitir(1, "VCS-1").unwrap();
    let raiz = [7u8; 32];
    assert_erro_rep(
        r.criar_dist(credito_pda(1), 6_500, raiz, 4),
        R::CreditoNaoVendido,
    );
    r.c.vender(1).unwrap();

    assert_erro_rep(
        r.criar_dist(credito_pda(1), 6_499, raiz, 4),
        R::IndustriaForaDaFaixa,
    );
    assert_erro_rep(
        r.criar_dist(credito_pda(1), 7_901, raiz, 4),
        R::IndustriaForaDaFaixa,
    );
    assert_erro_rep(
        r.criar_dist(credito_pda(1), 6_500, raiz, 0),
        R::NumFolhasInvalido,
    );
    assert_erro_rep(
        r.criar_dist(credito_pda(1), 6_500, raiz, 4_097),
        R::NumFolhasInvalido,
    );
    r.criar_dist(credito_pda(1), 7_900, raiz, 4_096).unwrap();
    assert_eq!(r.dist(credito_pda(1)).ecolchain_centavos, 50);
    assert!(
        r.criar_dist(credito_pda(1), 6_500, raiz, 4).is_err(),
        "segunda distribuição do mesmo crédito"
    );

    // Conta que não é Credito do ecol_credito.
    assert!(r.criar_dist(config_rep(), 6_500, raiz, 4).is_err());
}

#[test]
fn percentuais_validados_e_distribuicao_guarda_copia() {
    let mut r = Rep::novo();
    let (credito, direitos) = r.financiada(1);
    let mut p = r.params();
    p.bps.industria_max = 8_000; // EcolChain com 0%
    assert_erro_rep(r.update(p), R::PercentuaisInvalidos);
    let mut p = r.params();
    p.zupy = p.operador;
    assert_erro_rep(r.update(p), R::ChavesDeConfigInvalidas);

    let mut p = r.params();
    p.bps.coletor = 1_000;
    p.bps.industria_max = 7_400;
    r.update(p).unwrap();
    // A distribuição já criada mantém os tetos originais.
    assert_eq!(r.dist(credito).total_por_papel[0], 250);
    r.resgatar(credito, &direitos, 0).unwrap();
}

#[test]
fn participante_desativado_mantem_direito_adquirido() {
    let mut r = Rep::novo();
    let (credito, direitos) = r.financiada(1);
    let coletor = r.coletor.pubkey();
    r.c.amb.set_ativo(coletor, false).unwrap();
    r.resgatar(credito, &direitos, 0).unwrap();
}

#[test]
fn pausa_e_init() {
    let mut r = Rep::novo();
    let (credito, direitos) = r.financiada(1);
    let admin = r.c.amb.admin.insecure_clone();
    let ix = r.ix(
        ixs::AdminSetPausado { pausado: true },
        ctas::AdminUpdateConfig {
            admin: admin.pubkey(),
            config: config_rep(),
            event_authority: event_authority_rep(),
            program: ecol_reparticao::id(),
        },
    );
    r.c.amb.enviar(ix, &[&admin]).unwrap();
    assert_erro_rep(r.resgatar(credito, &direitos, 0), R::ProgramaPausado);
    let ua = r.c.amb.upgrade_authority.insecure_clone();
    assert!(r.init(&ua).is_err(), "config já existe");
}

#[test]
fn init_por_intruso_falha() {
    let mut c = Cenario::novo();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/ecol_reparticao.so"
    ));
    c.amb.svm.add_program(ecol_reparticao::id(), bytes).unwrap();
    let mut r = Rep {
        c,
        zupy: Keypair::new(),
        coletor: Keypair::new(),
    };
    assert_erro_rep(r.init(&Keypair::new()), R::NaoEhUpgradeAuthority);
}
