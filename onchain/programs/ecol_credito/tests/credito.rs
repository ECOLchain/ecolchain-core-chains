//! `ecol_credito` rodando junto com o `ecol_lote` (CPI real de agregação).
#[path = "../../ecol_lote/tests/common/mod.rs"]
mod common;

use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    common::*,
    ecol_credito::{
        accounts as ctas, error::EcolCreditoError as C, instruction as ixs, Credito, EstadoCredito,
        PadraoRegistro, SEED_AUTORIDADE, SEED_CONFIG, SEED_CREDITO, SEED_SERIAL,
    },
    ecol_lote::{error::EcolLoteError, EstadoLote, Material},
    litesvm::types::TransactionMetadata,
    solana_keypair::Keypair,
    solana_signer::Signer,
    solana_transaction::InstructionError,
    solana_transaction_error::TransactionError,
};

fn cpda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ecol_credito::id()).0
}
fn config_credito() -> Pubkey {
    cpda(&[SEED_CONFIG])
}
fn credito_pda(id: u64) -> Pubkey {
    cpda(&[SEED_CREDITO, &id.to_le_bytes()])
}
fn autoridade() -> Pubkey {
    cpda(&[SEED_AUTORIDADE])
}
fn event_authority_credito() -> Pubkey {
    cpda(&[b"__event_authority"])
}
fn sha256(b: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b]).to_bytes()
}
fn serial_pda(padrao: PadraoRegistro, serial: &str) -> Pubkey {
    cpda(&[SEED_SERIAL, &[padrao as u8], &sha256(serial.as_bytes())])
}

#[track_caller]
fn assert_erro_credito(res: Result<TransactionMetadata, TransactionError>, e: C) {
    let esperado: u32 = e.into();
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, esperado)
        }
        outro => panic!("esperava {esperado}, veio {outro:?}"),
    }
}

struct Cenario {
    amb: Ambiente,
    registrador: Keypair,
    comprador: Keypair,
}

impl Cenario {
    fn novo() -> Self {
        let mut amb = Ambiente::novo();
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/ecol_credito.so"
        ));
        amb.svm.add_program(ecol_credito::id(), bytes).unwrap();
        // Upgrade authority no ProgramData (o LiteSVM cria vazia).
        let pd = Pubkey::find_program_address(
            &[ecol_credito::id().as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        )
        .0;
        let mut conta = amb.svm.get_account(&pd).unwrap();
        conta.data[12] = 1;
        conta.data[13..45].copy_from_slice(amb.upgrade_authority.pubkey().as_ref());
        amb.svm.set_account(pd, conta).unwrap();

        // O ecol_lote passa a aceitar a PDA do ecol_credito como autoridade de agregação.
        let admin = amb.admin.insecure_clone();
        let mut p = amb.params();
        p.credito_autoridade = autoridade();
        amb.update_global(&admin, p).unwrap();

        let mut c = Cenario {
            amb,
            registrador: Keypair::new(),
            comprador: Keypair::new(),
        };
        let ua = c.amb.upgrade_authority.insecure_clone();
        c.init(&ua).unwrap();
        c
    }

    fn ix(&self, data: impl InstructionData, contas: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(
            ecol_credito::id(),
            &data.data(),
            contas.to_account_metas(None),
        )
    }

    fn init(&mut self, ua: &Keypair) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitConfig {
                admin: self.amb.admin.pubkey(),
                operador: self.amb.operador.pubkey(),
                registrador: self.registrador.pubkey(),
            },
            ctas::AdminInitConfig {
                payer: self.amb.patrocinador.pubkey(),
                upgrade_authority: ua.pubkey(),
                config: config_credito(),
                este_programa: ecol_credito::id(),
                program_data: Pubkey::find_program_address(
                    &[ecol_credito::id().as_ref()],
                    &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
                )
                .0,
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[ua])
    }

    fn criar_credito_por(
        &mut self,
        operador: &Keypair,
        id: u64,
        material: Material,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorCreateCredito {
                credito_id: id,
                padrao: PadraoRegistro::Verra,
                material,
                vintage: 2026,
                metodologia_hash: h("metodologia-plastico"),
            },
            ctas::OperadorCreateCredito {
                payer: self.amb.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[operador])
    }

    fn criar_credito(&mut self, id: u64) {
        let op = self.amb.operador.insecure_clone();
        self.criar_credito_por(&op, id, Material::Plastico).unwrap();
    }

    fn ix_add_lotes(&self, id: u64, lotes: &[Pubkey]) -> Instruction {
        let mut ix = self.ix(
            ixs::OperadorAddLotes {},
            ctas::OperadorAddLotes {
                operador: self.amb.operador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                autoridade: autoridade(),
                lote_config: config_pda(),
                lote_event_authority: event_authority(),
                ecol_lote_program: ecol_lote::id(),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        ix.accounts
            .extend(lotes.iter().map(|l| AccountMeta::new(*l, false)));
        ix
    }

    fn add_lotes(
        &mut self,
        id: u64,
        lotes: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_add_lotes(id, lotes);
        let op = self.amb.operador.insecure_clone();
        self.amb.enviar(ix, &[&op])
    }

    fn submeter(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let op = self.amb.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSubmitCredito {},
            ctas::OperadorSubmitCredito {
                operador: op.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[&op])
    }

    fn emitir_com(
        &mut self,
        registrador: &Keypair,
        id: u64,
        serial: &str,
        serial_hash: [u8; 32],
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::RegistradorRecordEmissao {
                serial: serial.to_string(),
                serial_hash,
                tco2e_milli: 1_000,
            },
            ctas::RegistradorRecordEmissao {
                payer: self.amb.patrocinador.pubkey(),
                registrador: registrador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                serial_registro: cpda(&[SEED_SERIAL, &[PadraoRegistro::Verra as u8], &serial_hash]),
                system_program: system_program::ID,
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[registrador])
    }

    fn emitir(&mut self, id: u64, serial: &str) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        self.emitir_com(&r, id, serial, sha256(serial.as_bytes()))
    }

    fn contas_registrador(
        &self,
        registrador: &Keypair,
        id: u64,
    ) -> ctas::RegistradorRecordRejeicao {
        ctas::RegistradorRecordRejeicao {
            registrador: registrador.pubkey(),
            config: config_credito(),
            credito: credito_pda(id),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        }
    }

    fn rejeitar(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        let ix = self.ix(
            ixs::RegistradorRecordRejeicao {
                motivo_hash: h("adicionalidade"),
            },
            self.contas_registrador(&r, id),
        );
        self.amb.enviar(ix, &[&r])
    }

    fn aposentar(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let r = self.registrador.insecure_clone();
        let ix = self.ix(
            ixs::RegistradorRecordAposentadoria {
                prova_hash: h("certificado-aposentadoria"),
            },
            self.contas_registrador(&r, id),
        );
        self.amb.enviar(ix, &[&r])
    }

    fn vender_com(
        &mut self,
        comprador: &Keypair,
        id: u64,
        valor: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let op = self.amb.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRecordVenda {
                valor_centavos: valor,
            },
            ctas::OperadorRecordVenda {
                operador: op.pubkey(),
                comprador: comprador.pubkey(),
                config: config_credito(),
                credito: credito_pda(id),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        );
        self.amb.enviar(ix, &[&op, comprador])
    }

    fn vender(&mut self, id: u64) -> Result<TransactionMetadata, TransactionError> {
        let c = self.comprador.insecure_clone();
        self.vender_com(&c, id, 5_000)
    }

    fn credito(&self, id: u64) -> Credito {
        let conta = self.amb.svm.get_account(&credito_pda(id)).unwrap();
        Credito::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Lote `lote_id` da cooperativa principal até `Reciclado`.
    fn lote_reciclado(&mut self, lote_id: u64) -> Pubkey {
        let lote = self.amb.lote_ate_recebido(lote_id);
        self.amb.liberar(lote).unwrap();
        lote
    }
}

#[test]
fn ciclo_completo_do_credito() {
    let mut c = Cenario::novo();
    let lotes: Vec<Pubkey> = (1..=3).map(|i| c.lote_reciclado(i)).collect();
    c.criar_credito(7);
    c.add_lotes(7, &lotes).unwrap();

    let cr = c.credito(7);
    assert_eq!((cr.qtd_lotes, cr.peso_total_g), (3, 3 * PESO_G));
    // Auditor reconstrói o hash encadeado a partir dos lotes.
    let mut esperado = [0u8; 32];
    for l in &lotes {
        esperado =
            solana_sha256_hasher::hashv(&[&esperado, l.as_ref(), &PESO_G.to_le_bytes()]).to_bytes();
    }
    assert_eq!(cr.lotes_hash, esperado);
    for l in &lotes {
        let lote = c.amb.lote(*l);
        assert_eq!(
            (lote.estado, lote.credito),
            (EstadoLote::Agregado, credito_pda(7))
        );
    }

    c.submeter(7).unwrap();
    c.emitir(7, "VCS-1234-2026-BR-001").unwrap();
    let cr = c.credito(7);
    assert_eq!(
        (cr.estado, cr.serial.as_str(), cr.tco2e_milli),
        (EstadoCredito::Emitido, "VCS-1234-2026-BR-001", 1_000)
    );

    c.vender(7).unwrap();
    let cr = c.credito(7);
    assert_eq!(
        (cr.comprador, cr.valor_venda_centavos),
        (c.comprador.pubkey(), 5_000)
    );
    c.aposentar(7).unwrap();
    let cr = c.credito(7);
    assert_eq!(cr.estado, EstadoCredito::Aposentado);
    // criação(1) + 3 lotes + submissão(5) + emissão(6,7) + venda(8,9) + aposentadoria(10,11)
    assert_eq!(cr.seq, 11);
}

#[test]
fn lote_nao_entra_em_dois_creditos() {
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.criar_credito(2);
    c.add_lotes(1, &[lote]).unwrap();
    assert_erro(c.add_lotes(2, &[lote]), EcolLoteError::TransicaoInvalida);
    // Nem repetido na mesma transação.
    let lote2 = c.lote_reciclado(2);
    assert_erro(
        c.add_lotes(2, &[lote2, lote2]),
        EcolLoteError::TransicaoInvalida,
    );
    assert_eq!(c.amb.lote(lote2).estado, EstadoLote::Reciclado);
}

#[test]
fn so_lotes_reciclados_do_mesmo_material() {
    let mut c = Cenario::novo();
    let recebido = c.amb.lote_ate_recebido(1);
    c.criar_credito(1);
    assert_erro(
        c.add_lotes(1, &[recebido]),
        EcolLoteError::TransicaoInvalida,
    );

    let op = c.amb.operador.insecure_clone();
    c.criar_credito_por(&op, 2, Material::Papel).unwrap();
    c.amb.liberar(recebido).unwrap();
    assert_erro_credito(c.add_lotes(2, &[recebido]), C::MaterialDiferente);
}

#[test]
fn conta_que_nao_e_lote_e_rejeitada() {
    let mut c = Cenario::novo();
    c.criar_credito(1);
    c.criar_credito(2);
    // Conta de outro programa (um crédito) no lugar de um lote.
    assert_erro_credito(c.add_lotes(1, &[credito_pda(2)]), C::LoteDeOutroPrograma);
    // Conta do ecol_lote que não é Lote (o GlobalConfig): discriminator errado.
    assert!(c.add_lotes(1, &[config_pda()]).is_err());
}

#[test]
fn limites_de_quantidade_e_de_estado() {
    let mut c = Cenario::novo();
    c.criar_credito(1);
    assert_erro_credito(c.add_lotes(1, &[]), C::QuantidadeDeLotesInvalida);
    let muitos: Vec<Pubkey> = (0..11).map(|_| Pubkey::new_unique()).collect();
    assert_erro_credito(c.add_lotes(1, &muitos), C::QuantidadeDeLotesInvalida);
    assert_erro_credito(c.submeter(1), C::CreditoVazio);

    let lote = c.lote_reciclado(1);
    c.add_lotes(1, &[lote]).unwrap();
    c.submeter(1).unwrap();
    let outro = c.lote_reciclado(2);
    assert_erro_credito(c.add_lotes(1, &[outro]), C::TransicaoInvalida);
    assert_erro_credito(c.vender(1), C::TransicaoInvalida);
    assert_erro_credito(c.aposentar(1), C::TransicaoInvalida);
}

#[test]
fn serial_unico_e_conferido() {
    let mut c = Cenario::novo();
    for (id, lote_id) in [(1u64, 1u64), (2, 2)] {
        let lote = c.lote_reciclado(lote_id);
        c.criar_credito(id);
        c.add_lotes(id, &[lote]).unwrap();
        c.submeter(id).unwrap();
    }
    let r = c.registrador.insecure_clone();
    // Hash que não é do serial informado.
    assert_erro_credito(
        c.emitir_com(&r, 1, "VCS-1", sha256(b"VCS-2")),
        C::SerialInvalido,
    );
    assert_erro_credito(c.emitir_com(&r, 1, "", sha256(b"")), C::SerialInvalido);
    c.emitir(1, "VCS-1").unwrap();
    assert!(c
        .amb
        .svm
        .get_account(&serial_pda(PadraoRegistro::Verra, "VCS-1"))
        .is_some());
    // Mesmo serial para outro crédito: a PDA do serial já existe.
    assert!(c.emitir(2, "VCS-1").is_err());
    assert_eq!(c.credito(2).estado, EstadoCredito::EmValidacao);
}

#[test]
fn rejeicao_encerra_sem_emissao() {
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.add_lotes(1, &[lote]).unwrap();
    c.submeter(1).unwrap();
    c.rejeitar(1).unwrap();
    assert_eq!(c.credito(1).estado, EstadoCredito::Rejeitado);
    assert_erro_credito(c.emitir(1, "VCS-9"), C::TransicaoInvalida);
    // Lotes de crédito rejeitado continuam agregados (não são reaproveitados).
    assert_eq!(c.amb.lote(lote).estado, EstadoLote::Agregado);
}

#[test]
fn papeis_conferidos() {
    let mut c = Cenario::novo();
    let intruso = Keypair::new();
    assert_erro_credito(
        c.criar_credito_por(&intruso, 1, Material::Plastico),
        C::NaoEhOperador,
    );

    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.add_lotes(1, &[lote]).unwrap();
    c.submeter(1).unwrap();
    assert_erro_credito(
        c.emitir_com(&intruso, 1, "VCS-1", sha256(b"VCS-1")),
        C::NaoEhRegistrador,
    );
    c.emitir(1, "VCS-1").unwrap();
    // O registrador não pode ser o comprador.
    let r = c.registrador.insecure_clone();
    assert_erro_credito(c.vender_com(&r, 1, 5_000), C::ChavesDeConfigInvalidas);
    let comprador = c.comprador.insecure_clone();
    assert_erro_credito(c.vender_com(&comprador, 1, 0), C::ParametroInvalido);
    c.vender(1).unwrap();
}

#[test]
fn autoridade_do_credito_so_age_via_programa() {
    // Sem a PDA do ecol_credito, ninguém marca lote como agregado direto no ecol_lote.
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    let intruso = Keypair::new();
    assert_erro(
        c.amb.agregar_por(&intruso, lote, Pubkey::new_unique()),
        EcolLoteError::NaoEhAutoridadeCredito,
    );
}

#[test]
fn init_exige_upgrade_authority_e_pausa_bloqueia() {
    let mut c = Cenario::novo();
    let ua = c.amb.upgrade_authority.insecure_clone();
    assert!(c.init(&ua).is_err(), "config já existe");

    let admin = c.amb.admin.insecure_clone();
    let ix = c.ix(
        ixs::AdminSetPausado { pausado: true },
        ctas::AdminUpdateConfig {
            admin: admin.pubkey(),
            config: config_credito(),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        },
    );
    c.amb.enviar(ix, &[&admin]).unwrap();
    let op = c.amb.operador.insecure_clone();
    assert_erro_credito(
        c.criar_credito_por(&op, 1, Material::Plastico),
        C::ProgramaPausado,
    );

    let ix = c.ix(
        ixs::AdminUpdateConfig {
            operador: c.registrador.pubkey(),
            registrador: c.registrador.pubkey(),
        },
        ctas::AdminUpdateConfig {
            admin: admin.pubkey(),
            config: config_credito(),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        },
    );
    assert_erro_credito(c.amb.enviar(ix, &[&admin]), C::ChavesDeConfigInvalidas);
}

#[test]
fn init_por_intruso_falha() {
    // SVM sem config do crédito: quem não é a upgrade authority não inicializa.
    let mut amb = Ambiente::novo();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/ecol_credito.so"
    ));
    amb.svm.add_program(ecol_credito::id(), bytes).unwrap();
    let mut c = Cenario {
        amb,
        registrador: Keypair::new(),
        comprador: Keypair::new(),
    };
    let intruso = Keypair::new();
    assert_erro_credito(c.init(&intruso), C::NaoEhUpgradeAuthority);
}
