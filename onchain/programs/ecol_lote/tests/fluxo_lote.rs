//! Cenários de ponta a ponta da máquina de estados do lote.
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{
        error::EcolLoteError as E, DecisaoDisputa, EstadoLote, EtapaDisputavel, MotivoDisputa,
    },
    solana_signer::Signer,
};

#[test]
fn ciclo_completo_ate_agregado() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);

    amb.criar_lote(1).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Criado);
    // seq: início da montagem + origem vinculada + fechamento (LoteCriado) + pesagem.
    assert_eq!(
        (l.peso_g, l.seq, l.criado_em, l.ts_pesagem),
        (PESO_G, 4, T0, T0)
    );
    assert!(l.asset_queimavel);
    assert_eq!((l.qtd_entregas, l.peso_entregas_g), (1, PESO_G));
    assert_eq!(l.balanca_origem, balanca_pda(&amb.balanca_coop.pubkey()));
    let asset = asset_pda(&lote);
    assert_eq!(l.asset, asset);
    assert_eq!(dono_do_asset(&amb.svm, &asset), amb.cooperativa.pubkey());

    amb.anunciar(lote).unwrap();
    assert_eq!(
        amb.lote(lote).estado,
        EstadoLote::Anunciado {
            prazo_leilao: T0 + 3600
        }
    );

    amb.vender(lote).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Vendido);
    assert_eq!(l.industria, amb.industria.pubkey());
    assert_eq!(l.valor_centavos, 25_000);
    assert_eq!(l.prazo_entrega, T0 + PRAZO_ENTREGA_S);

    amb.retirar(lote).unwrap();
    assert_eq!(amb.lote(lote).transportador, amb.transportador.pubkey());
    assert_eq!(dono_do_asset(&amb.svm, &asset), amb.transportador.pubkey());

    // 1% de diferença: dentro da tolerância de 2%.
    amb.receber(lote, PESO_G - PESO_G / 100).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Recebido);
    assert!(!amb.lote(lote).recebimento_atestado);
    assert_eq!(dono_do_asset(&amb.svm, &asset), amb.industria.pubkey());

    amb.liberar(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);

    let credito = Pubkey::new_unique();
    let autoridade = amb.credito_autoridade.insecure_clone();
    amb.agregar_por(&autoridade, lote, credito).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Agregado);
    assert_eq!(l.credito, credito);
    // montagem(1) origem(2) fechamento(3,4) anúncio(5) venda(6,7) retirada(8) recebimento(9)
    // liberação(10,11) agregação(12)
    assert_eq!(l.seq, 12);

    // Estado terminal: nada mais anda.
    assert_erro(
        amb.agregar_por(&autoridade, lote, credito),
        E::TransicaoInvalida,
    );
}

#[test]
fn sem_lance_e_reanuncio() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.criar_lote(2).unwrap();
    amb.anunciar(lote).unwrap();

    assert_erro(amb.encerrar_sem_lance(lote), E::LeilaoEmAndamento);
    amb.set_tempo(T0 + 3600);
    amb.encerrar_sem_lance(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::SemLance);

    // Sem anúncio vigente não há venda.
    assert_erro(amb.vender(lote), E::TransicaoInvalida);

    let coop = amb.cooperativa.insecure_clone();
    assert_erro(
        amb.anunciar_por(&coop, lote, T0 + 3600, 20_000),
        E::PrazoInvalido,
    );
    amb.anunciar_por(&coop, lote, T0 + 7200, 20_000).unwrap();
    assert_eq!(amb.lote(lote).preco_minimo_centavos, 20_000);

    let (ind, inter) = (
        amb.industria.insecure_clone(),
        amb.intermediador.insecure_clone(),
    );
    assert_erro(
        amb.vender_com(&ind, &inter, lote, 19_999),
        E::ValorAbaixoDoMinimo,
    );
    amb.vender_com(&ind, &inter, lote, 20_000).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Vendido);
}

#[test]
fn divergencia_de_peso_abre_disputa_e_arbitro_reclassifica() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 3);
    amb.criar_lote(3).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    amb.retirar(lote).unwrap();

    // 10% a menos: acima da tolerância de 2%.
    amb.receber(lote, 900_000).unwrap();
    let l = amb.lote(lote);
    assert_eq!(
        l.estado,
        EstadoLote::EmDisputa {
            motivo: MotivoDisputa::DivergenciaPeso,
            origem: EtapaDisputavel::Recebido
        }
    );
    assert_eq!(l.peso_recebido_g, 900_000);
    assert_erro(amb.liberar(lote), E::TransicaoInvalida);

    let arbitro = amb.arbitro.insecure_clone();
    amb.resolver(
        &arbitro,
        None,
        lote,
        DecisaoDisputa::Prosseguir {
            novo_peso_g: Some(900_000),
        },
    )
    .unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Recebido);
    assert_eq!(l.peso_g, 900_000);

    amb.liberar(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);
}

#[test]
fn prazo_expirado_crank_e_reembolso() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 4);
    amb.criar_lote(4).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();

    assert_erro(amb.crank_prazo(lote), E::PrazoNaoExpirado);
    amb.set_tempo(T0 + PRAZO_ENTREGA_S + 1);
    amb.crank_prazo(lote).unwrap();
    assert_eq!(
        amb.lote(lote).estado,
        EstadoLote::EmDisputa {
            motivo: MotivoDisputa::PrazoExpirado,
            origem: EtapaDisputavel::Vendido
        }
    );

    let arbitro = amb.arbitro.insecure_clone();
    let reembolso = DecisaoDisputa::Reembolsar {
        ref_hash: h("pix-estorno"),
    };
    // Reembolso exige o intermediador atestando a devolução.
    assert_erro(
        amb.resolver(&arbitro, None, lote, reembolso),
        E::NaoEhIntermediador,
    );
    let impostor = solana_keypair::Keypair::new();
    assert_erro(
        amb.resolver(&arbitro, Some(&impostor), lote, reembolso),
        E::NaoEhIntermediador,
    );

    let inter = amb.intermediador.insecure_clone();
    amb.resolver(&arbitro, Some(&inter), lote, reembolso)
        .unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reembolsado);
    assert_erro(amb.retirar(lote), E::TransicaoInvalida);
}

#[test]
fn prosseguir_depois_do_prazo_renova_o_prazo_de_entrega() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 5);
    amb.criar_lote(5).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    amb.retirar(lote).unwrap();

    let t1 = T0 + PRAZO_ENTREGA_S + 10;
    amb.set_tempo(t1);
    amb.crank_prazo(lote).unwrap();

    let arbitro = amb.arbitro.insecure_clone();
    amb.resolver(
        &arbitro,
        None,
        lote,
        DecisaoDisputa::Prosseguir { novo_peso_g: None },
    )
    .unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::EmTransporte);
    assert_eq!(l.prazo_entrega, t1 + PRAZO_ENTREGA_S);
    assert_eq!(l.peso_g, PESO_G);
    // Com o prazo renovado, o crank não devolve o lote à disputa.
    assert_erro(amb.crank_prazo(lote), E::PrazoNaoExpirado);
}

#[test]
fn partes_abrem_disputa_manualmente() {
    let mut amb = Ambiente::novo();
    let lote = amb.lote_ate_recebido(6);

    let ind = amb.industria.insecure_clone();
    assert_erro(
        amb.abrir_disputa(&ind, lote, MotivoDisputa::PrazoExpirado),
        E::ParametroInvalido,
    );
    amb.abrir_disputa(&ind, lote, MotivoDisputa::ClassificacaoContestada)
        .unwrap();
    assert_eq!(
        amb.lote(lote).estado,
        EstadoLote::EmDisputa {
            motivo: MotivoDisputa::ClassificacaoContestada,
            origem: EtapaDisputavel::Recebido
        }
    );
    // Já em disputa: não abre outra.
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(
        amb.abrir_disputa(&coop, lote, MotivoDisputa::Outro),
        E::TransicaoInvalida,
    );
}
