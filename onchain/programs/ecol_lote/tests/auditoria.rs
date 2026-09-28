//! Regressões dos achados da auditoria de 2026-09-28 (checklists do solana-ai-kit).
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{
        error::EcolLoteError as E, DecisaoDisputa, EstadoLote, ModoOperacao, MotivoDisputa, Papel,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

/// Achado 1 (Alta): disputa perpétua travando o escrow.
#[test]
fn etapa_julgada_nao_pode_ser_redisputada_pelas_partes() {
    let mut amb = Ambiente::novo();
    let lote = amb.lote_ate_recebido(1);
    let (ind, coop, arb) = (
        amb.industria.insecure_clone(),
        amb.cooperativa.insecure_clone(),
        amb.arbitro.insecure_clone(),
    );
    amb.abrir_disputa(&ind, lote, MotivoDisputa::Outro).unwrap();
    amb.resolver(
        &arb,
        None,
        lote,
        DecisaoDisputa::Prosseguir { novo_peso_g: None },
    )
    .unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Recebido);

    assert_erro(
        amb.abrir_disputa(&ind, lote, MotivoDisputa::Outro),
        E::DisputaJaJulgada,
    );
    assert_erro(
        amb.abrir_disputa(&coop, lote, MotivoDisputa::ClassificacaoContestada),
        E::DisputaJaJulgada,
    );
    amb.liberar(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);
}

#[test]
fn julgamento_numa_etapa_nao_impede_disputa_na_seguinte() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    let (coop, ind, arb) = (
        amb.cooperativa.insecure_clone(),
        amb.industria.insecure_clone(),
        amb.arbitro.insecure_clone(),
    );
    amb.abrir_disputa(&coop, lote, MotivoDisputa::Outro)
        .unwrap();
    amb.resolver(
        &arb,
        None,
        lote,
        DecisaoDisputa::Prosseguir { novo_peso_g: None },
    )
    .unwrap();
    amb.retirar(lote).unwrap();
    amb.receber(lote, PESO_G).unwrap();
    amb.abrir_disputa(&ind, lote, MotivoDisputa::ClassificacaoContestada)
        .unwrap();
}

/// Achados 1(a) e 6: resolver e liquidar de uma vez, inclusive com liberação parcial.
#[test]
fn arbitro_libera_parcialmente_com_peso_reclassificado() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    amb.retirar(lote).unwrap();
    amb.receber(lote, 800_000).unwrap(); // 20% a menos → disputa automática
    let seq_antes = amb.lote(lote).seq;

    let (arb, inter) = (
        amb.arbitro.insecure_clone(),
        amb.intermediador.insecure_clone(),
    );
    let liberar = |v| DecisaoDisputa::Liberar {
        valor_liberado_centavos: v,
        novo_peso_g: Some(800_000),
        ref_hash: h("acordo-arbitral"),
    };
    assert_erro(
        amb.resolver(&arb, None, lote, liberar(20_000)),
        E::NaoEhIntermediador,
    );
    assert_erro(
        amb.resolver(&arb, Some(&inter), lote, liberar(25_001)),
        E::ValorLiberadoInvalido,
    );
    amb.resolver(&arb, Some(&inter), lote, liberar(20_000))
        .unwrap();

    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Reciclado);
    assert_eq!((l.peso_g, l.valor_liberado_centavos), (800_000, 20_000));
    // transição + peso + liberação + reembolso do restante
    assert_eq!(l.seq, seq_antes + 4);
}

#[test]
fn liberar_so_depois_da_entrega() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    let (coop, arb, inter) = (
        amb.cooperativa.insecure_clone(),
        amb.arbitro.insecure_clone(),
        amb.intermediador.insecure_clone(),
    );
    amb.abrir_disputa(&coop, lote, MotivoDisputa::Outro)
        .unwrap();
    let d = DecisaoDisputa::Liberar {
        valor_liberado_centavos: 25_000,
        novo_peso_g: None,
        ref_hash: h("x"),
    };
    assert_erro(
        amb.resolver(&arb, Some(&inter), lote, d),
        E::TransicaoInvalida,
    );
}

/// Achado 2 (Alta): quem paga a taxa não tem autoridade nenhuma.
#[test]
fn fee_payer_nao_age_como_operador() {
    let mut amb = Ambiente::novo();
    let sybil = Keypair::new().pubkey();
    let patrocinador = amb.patrocinador.insecure_clone();
    let ix = amb.ix(
        ecol_lote::instruction::OperadorRegisterParticipante {
            carteira: sybil,
            papel: Papel::Industria,
            kyc_hash: h("k"),
            nome: "Sybil".into(),
        },
        ecol_lote::accounts::OperadorRegisterParticipante {
            payer: patrocinador.pubkey(),
            operador: patrocinador.pubkey(),
            config: config_pda(),
            participante: participante_pda(&sybil),
            system_program: anchor_lang::solana_program::system_program::ID,
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    assert_erro(amb.enviar(ix, &[]), E::NaoEhOperador);
}

/// Achado 3 (Média): venda exige o operador (apurador do leilão).
#[test]
fn venda_exige_operador_apurador() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    let (ind, inter) = (
        amb.industria.insecure_clone(),
        amb.intermediador.insecure_clone(),
    );
    assert_erro(
        amb.vender_completo(&ind, &Keypair::new(), &inter, lote, 25_000),
        E::NaoEhOperador,
    );
    amb.vender(lote).unwrap();
}

/// Achado 4 (Média): pausa não derruba lotes por prazo.
#[test]
fn crank_bloqueado_fora_do_normal_e_com_carencia_na_retomada() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();

    amb.set_modo(ModoOperacao::SomenteDisputas).unwrap();
    let t = T0 + PRAZO_ENTREGA_S + 1;
    amb.set_tempo(t);
    assert_erro(amb.crank_prazo(lote), E::ProgramaPausado);

    amb.set_modo(ModoOperacao::Normal).unwrap();
    assert_eq!(amb.config().retomado_em, t);
    assert_erro(amb.crank_prazo(lote), E::EmCarencia);
    // Na carência, as partes retomam normalmente.
    amb.set_tempo(t + 48 * 3600 - 1);
    assert_erro(amb.crank_prazo(lote), E::EmCarencia);
    amb.set_tempo(t + 48 * 3600);
    amb.crank_prazo(lote).unwrap();
}

/// Achado 5 (Média): participante desativado não assume novos compromissos.
#[test]
fn cooperativa_inativa_nao_anuncia_vende_nem_entrega() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.pubkey();
    let (l1, l2, l3) = (lote_pda(&coop, 1), lote_pda(&coop, 2), lote_pda(&coop, 3));
    for id in 1..=3 {
        amb.criar_lote(id).unwrap();
    }
    amb.anunciar(l2).unwrap();
    amb.anunciar(l3).unwrap();
    amb.vender(l3).unwrap();

    amb.set_ativo(coop, false).unwrap();
    assert_erro(amb.anunciar(l1), E::ParticipanteInativo);
    assert_erro(amb.vender(l2), E::ParticipanteInativo);
    assert_erro(amb.retirar(l3), E::ParticipanteInativo);

    // Negócio com dinheiro já depositado pode ser encerrado.
    amb.set_ativo(coop, true).unwrap();
    amb.retirar(l3).unwrap();
    amb.set_ativo(coop, false).unwrap();
    let ind = amb.industria.pubkey();
    amb.set_ativo(ind, false).unwrap();
    amb.receber(l3, PESO_G).unwrap();
    amb.liberar(l3).unwrap();
}

/// Achado 7 (Baixa): uma chave, um papel.
#[test]
fn chaves_de_config_distintas_e_reservadas() {
    let mut amb = Ambiente::novo();
    let admin = amb.admin.insecure_clone();
    let mut p = amb.params();
    p.arbitro = p.intermediador;
    assert_erro(amb.update_global(&admin, p), E::ChavesDeConfigInvalidas);
    let mut p = amb.params();
    p.operador = Pubkey::default();
    assert_erro(amb.update_global(&admin, p), E::ChavesDeConfigInvalidas);
    let mut p = amb.params();
    p.arbitro = admin.pubkey();
    assert_erro(amb.update_global(&admin, p), E::ChavesDeConfigInvalidas);

    let inter = amb.intermediador.pubkey();
    assert_erro(amb.registrar(inter, Papel::Industria), E::CarteiraReservada);
}

/// Achado 8 (Baixa): troca de admin em dois passos e teto do prazo.
#[test]
fn troca_de_admin_em_dois_passos() {
    let mut amb = Ambiente::novo();
    let (admin, novo, intruso) = (amb.admin.insecure_clone(), Keypair::new(), Keypair::new());
    let propor = |amb: &Ambiente, quem: &Keypair, alvo: Pubkey| {
        amb.ix(
            ecol_lote::instruction::AdminProposeAdmin { novo_admin: alvo },
            ecol_lote::accounts::AdminUpdateGlobal {
                admin: quem.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    };
    let aceitar = |amb: &Ambiente, quem: &Keypair| {
        amb.ix(
            ecol_lote::instruction::AdminAcceptAdmin {},
            ecol_lote::accounts::AdminAcceptAdmin {
                novo_admin: quem.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    };
    let ix = propor(&amb, &intruso, intruso.pubkey());
    assert_erro(amb.enviar(ix, &[&intruso]), E::NaoEhAdmin);
    let ix = propor(&amb, &admin, novo.pubkey());
    amb.enviar(ix, &[&admin]).unwrap();
    // Proposta sozinha não muda o admin.
    assert_eq!(amb.config().admin, admin.pubkey());
    let ix = aceitar(&amb, &intruso);
    assert_erro(amb.enviar(ix, &[&intruso]), E::SemAdminPendente);
    let ix = aceitar(&amb, &novo);
    amb.enviar(ix, &[&novo]).unwrap();
    assert_eq!(amb.config().admin, novo.pubkey());
    assert_eq!(amb.config().admin_pendente, Pubkey::default());

    let p = amb.params();
    assert_erro(amb.update_global(&admin, p.clone()), E::NaoEhAdmin);
    let mut p = p;
    p.prazo_entrega_s = 180 * 24 * 3600 + 1;
    assert_erro(amb.update_global(&novo, p), E::ParametroInvalido);
}

/// Achado 9 (Baixa): liberação continua em `SomenteDisputas`.
#[test]
fn liberacao_permitida_em_somente_disputas() {
    let mut amb = Ambiente::novo();
    let lote = amb.lote_ate_recebido(1);
    amb.set_modo(ModoOperacao::SomenteDisputas).unwrap();
    amb.liberar(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);
}

/// Achado 10 (Baixa): anúncio com teto de duração, preço positivo e cancelamento.
#[test]
fn anuncio_limitado_e_cancelavel() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(
        amb.anunciar_por(&coop, lote, T0 + 3600, 0),
        E::ParametroInvalido,
    );
    assert_erro(
        amb.anunciar_por(&coop, lote, T0 + 90 * 24 * 3600 + 1, 1),
        E::PrazoInvalido,
    );
    amb.anunciar_por(&coop, lote, T0 + 90 * 24 * 3600, 1)
        .unwrap();

    let ix = amb.ix(
        ecol_lote::instruction::CooperativaCancelAnuncio {},
        ecol_lote::accounts::CooperativaListLote {
            cooperativa: coop.pubkey(),
            cooperativa_part: participante_pda(&coop.pubkey()),
            config: config_pda(),
            lote,
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[&coop]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::SemLance);
    amb.anunciar_por(&coop, lote, T0 + 3600, 30_000).unwrap();
}

/// Tolerância de peso é a vigente na venda, não a do momento do recebimento.
#[test]
fn tolerancia_fixada_na_venda() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    assert_eq!(amb.lote(lote).tolerancia_peso_bps, TOLERANCIA_BPS);

    let admin = amb.admin.insecure_clone();
    let mut p = amb.params();
    p.tolerancia_peso_bps = 2_000;
    amb.update_global(&admin, p).unwrap();

    amb.retirar(lote).unwrap();
    amb.receber(lote, 900_000).unwrap(); // 10%: dentro dos 20% novos, fora dos 2% da venda
    assert!(matches!(
        amb.lote(lote).estado,
        EstadoLote::EmDisputa { .. }
    ));
}
