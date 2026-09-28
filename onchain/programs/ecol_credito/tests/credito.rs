//! `ecol_credito` rodando junto com o `ecol_lote` (CPI real de agregação).
mod cenario;
#[path = "../../ecol_lote/tests/common/mod.rs"]
mod common;

use {
    anchor_lang::prelude::Pubkey,
    cenario::*,
    common::*,
    ecol_credito::{
        accounts as ctas, error::EcolCreditoError as C, instruction as ixs, EstadoCredito,
        PadraoRegistro,
    },
    ecol_lote::{error::EcolLoteError, EstadoLote},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

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
    c.criar_credito_por(&op, 2, PAPEL).unwrap();
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
    // Até alguém liberar, os lotes seguem agregados.
    assert_eq!(c.amb.lote(lote).estado, EstadoLote::Agregado);
}

#[test]
fn papeis_conferidos() {
    let mut c = Cenario::novo();
    let intruso = Keypair::new();
    assert_erro_credito(c.criar_credito_por(&intruso, 1, PLASTICO), C::NaoEhOperador);

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
    assert_erro_credito(c.criar_credito_por(&op, 1, PLASTICO), C::ProgramaPausado);

    let ix = c.ix(
        ixs::AdminUpdateConfig {
            operador: c.registrador.pubkey(),
            registrador: c.registrador.pubkey(),
            intermediador: c.amb.intermediador.pubkey(),
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

/// C1: lotes de crédito rejeitado voltam a `Reciclado` e entram em outro crédito.
#[test]
fn rejeicao_libera_lotes_para_outro_credito() {
    let mut c = Cenario::novo();
    let lotes: Vec<Pubkey> = (1..=2).map(|i| c.lote_reciclado(i)).collect();
    c.criar_credito(1);
    c.add_lotes(1, &lotes).unwrap();
    c.submeter(1).unwrap();
    // Antes da rejeição não há liberação.
    assert_erro_credito(c.liberar_lotes(1, &lotes), C::LiberacaoNaoPermitida);
    c.rejeitar(1).unwrap();

    c.liberar_lotes(1, &lotes).unwrap();
    for l in &lotes {
        let lote = c.amb.lote(*l);
        assert_eq!(
            (lote.estado, lote.credito),
            (EstadoLote::Reciclado, Pubkey::default())
        );
    }
    assert_eq!(c.credito(1).qtd_lotes_liberados, 2);
    // Liberar de novo: o lote já não pertence ao crédito.
    assert_erro_credito(c.liberar_lotes(1, &lotes[..1]), C::LoteDeOutroCredito);

    c.criar_credito(2);
    c.add_lotes(2, &lotes).unwrap();
    assert_eq!(c.amb.lote(lotes[0]).credito, credito_pda(2));
}

/// C1: crédito esquecido em formação libera os lotes após 180 dias, por qualquer um.
#[test]
fn credito_abandonado_libera_lotes_apos_prazo() {
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.add_lotes(1, &[lote]).unwrap();

    let prazo = c.credito(1).prazo_formacao;
    assert_eq!(prazo, T0 + 180 * 24 * 3600);
    c.amb.set_tempo(prazo);
    assert_erro_credito(c.liberar_lotes(1, &[lote]), C::PrazoNaoExpirado);
    c.amb.set_tempo(prazo + 1);
    c.liberar_lotes(1, &[lote]).unwrap();
    assert_eq!(c.credito(1).estado, EstadoCredito::Abandonado);
    assert_eq!(c.amb.lote(lote).estado, EstadoLote::Reciclado);
    // Abandonado não recebe mais lotes nem é submetido.
    assert_erro_credito(c.add_lotes(1, &[lote]), C::TransicaoInvalida);
    assert_erro_credito(c.submeter(1), C::TransicaoInvalida);
}

/// C1: só os lotes do próprio crédito, e nunca de crédito emitido.
#[test]
fn liberacao_nao_toca_lotes_de_outro_credito_nem_emitido() {
    let mut c = Cenario::novo();
    let (l1, l2) = (c.lote_reciclado(1), c.lote_reciclado(2));
    c.criar_credito(1);
    c.criar_credito(2);
    c.add_lotes(1, &[l1]).unwrap();
    c.add_lotes(2, &[l2]).unwrap();
    for id in [1, 2] {
        c.submeter(id).unwrap();
    }
    c.rejeitar(1).unwrap();
    c.emitir(2, "VCS-2").unwrap();
    // Lote do crédito 2 (emitido) passado junto com o crédito 1 (rejeitado).
    assert_erro_credito(c.liberar_lotes(1, &[l2]), C::LoteDeOutroCredito);
    assert_erro_credito(c.liberar_lotes(2, &[l2]), C::LiberacaoNaoPermitida);
    assert_eq!(c.amb.lote(l2).estado, EstadoLote::Agregado);
}

/// C1: ninguém além da PDA do ecol_credito desagrega direto no ecol_lote.
#[test]
fn desagregacao_direta_no_ecol_lote_e_bloqueada() {
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.add_lotes(1, &[lote]).unwrap();
    let intruso = Keypair::new();
    let ix = c.amb.ix(
        ecol_lote::instruction::CreditoMarkDesagregado {
            credito: credito_pda(1),
        },
        ecol_lote::accounts::CreditoMarkAgregado {
            credito_autoridade: intruso.pubkey(),
            config: config_pda(),
            lote,
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    assert_erro(
        c.amb.enviar(ix, &[&intruso]),
        EcolLoteError::NaoEhAutoridadeCredito,
    );
}

/// C2: o valor da venda do crédito precisa do intermediador de pagamento.
#[test]
fn venda_exige_intermediador() {
    let mut c = Cenario::novo();
    let lote = c.lote_reciclado(1);
    c.criar_credito(1);
    c.add_lotes(1, &[lote]).unwrap();
    c.submeter(1).unwrap();
    c.emitir(1, "VCS-1").unwrap();
    let comprador = c.comprador.insecure_clone();
    assert_erro_credito(
        c.vender_atestado(&comprador, &Keypair::new(), 1, 5_000),
        C::NaoEhIntermediador,
    );
    c.vender(1).unwrap();
    assert_eq!(c.credito(1).pagamento_ref_hash, h("pix-venda-credito"));
}

#[test]
fn credito_exige_material_cadastrado_e_ativo() {
    let mut c = Cenario::novo();
    let op = c.amb.operador.insecure_clone();
    // Material inexistente: a conta não existe e a instrução falha.
    assert!(c.criar_credito_por(&op, 1, 99).is_err());
    c.amb.atualizar_material(PAPEL, "Papel", false).unwrap();
    assert_erro(
        c.criar_credito_por(&op, 2, PAPEL),
        EcolLoteError::MaterialInativo,
    );
    c.criar_credito_por(&op, 3, PLASTICO).unwrap();
    assert_eq!(c.credito(3).material, PLASTICO);
}

#[test]
fn guardiao_pausa_e_so_admin_despausa() {
    let mut c = Cenario::novo();
    let admin = c.amb.admin.insecure_clone();
    let guardiao = Keypair::new();
    let set = c.ix(
        ixs::AdminSetGuardiao {
            guardiao: guardiao.pubkey(),
        },
        ctas::AdminUpdateConfig {
            admin: admin.pubkey(),
            config: config_credito(),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        },
    );
    c.amb.enviar(set, &[&admin]).unwrap();

    let pausar = |c: &Cenario, quem: &Keypair| {
        c.ix(
            ixs::GuardiaoPausar {},
            ctas::GuardiaoPausar {
                guardiao: quem.pubkey(),
                config: config_credito(),
                event_authority: event_authority_credito(),
                program: ecol_credito::id(),
            },
        )
    };
    let intruso = Keypair::new();
    let ix = pausar(&c, &intruso);
    assert_erro_credito(c.amb.enviar(ix, &[&intruso]), C::NaoEhGuardiao);
    let ix = pausar(&c, &guardiao);
    c.amb.enviar(ix, &[&guardiao]).unwrap();
    let op = c.amb.operador.insecure_clone();
    assert_erro_credito(c.criar_credito_por(&op, 1, PLASTICO), C::ProgramaPausado);

    let despausar = c.ix(
        ixs::AdminSetPausado { pausado: false },
        ctas::AdminUpdateConfig {
            admin: admin.pubkey(),
            config: config_credito(),
            event_authority: event_authority_credito(),
            program: ecol_credito::id(),
        },
    );
    c.amb.enviar(despausar, &[&admin]).unwrap();
    c.criar_credito_por(&op, 1, PLASTICO).unwrap();
}
