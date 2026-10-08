//! ADR 0012: lote à venda por preço fixo (a indústria compra sozinha) e lote vendido fora da plataforma.
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, EstadoLote, ModoRetirada, Papel},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const PRECO: u64 = 50_000; // R$ 500,00

/// Lote 1 fechado e posto à venda por `PRECO`.
fn lote_a_venda(amb: &mut Ambiente) -> anchor_lang::prelude::Pubkey {
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar_venda(lote, PRECO).unwrap();
    lote
}

#[test]
fn venda_direta_sem_prazo_e_com_preco() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    let l = amb.lote(lote);
    assert_eq!(
        l.estado,
        EstadoLote::Anunciado {
            prazo_leilao: i64::MAX
        }
    );
    assert!(l.venda_direta);
    assert_eq!(l.preco_minimo_centavos, PRECO);
    // Sem prazo, o operador não encerra a venda direta como "sem lance".
    assert_erro(amb.encerrar_sem_lance(lote), E::LeilaoEmAndamento);
}

#[test]
fn preco_zero_e_outra_cooperativa_falham() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    assert_erro(amb.anunciar_venda(lote, 0), E::ParametroInvalido);
    // Só a cooperativa do lote anuncia.
    let outra = Keypair::new();
    amb.registrar(outra.pubkey(), Papel::Cooperativa).unwrap();
    let ix = amb.ix_coop_lote(
        &outra.pubkey(),
        lote,
        ecol_lote::instruction::CooperativaAnunciarVenda {
            preco_centavos: PRECO,
        },
    );
    assert_erro(amb.enviar(ix, &[&outra]), E::NaoEhCooperativaDoLote);
}

#[test]
fn industria_compra_sozinha_pelo_preco_ou_mais() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    assert_erro(
        amb.comprar(lote, PRECO - 1, ModoRetirada::Terceiro),
        E::ValorAbaixoDoMinimo,
    );
    assert_erro(
        amb.comprar(lote, PRECO, ModoRetirada::Indefinido),
        E::ModoRetiradaInvalido,
    );
    // O leilão (três assinaturas) não vende um lote em venda direta.
    assert_erro(amb.vender(lote), E::LoteEmVendaDireta);

    amb.comprar(lote, PRECO + 1_000, ModoRetirada::Terceiro)
        .unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Vendido);
    assert_eq!(l.industria, amb.industria.pubkey());
    assert_eq!(l.valor_centavos, PRECO + 1_000);
    assert_eq!(l.modo_retirada, ModoRetirada::Terceiro);
    assert_eq!(l.escrow_ref_hash, [0; 32]);
    assert_eq!(l.prazo_entrega, T0 + PRAZO_ENTREGA_S);

    // Quem compra primeiro leva: a segunda compra falha.
    let outra = Keypair::new();
    amb.registrar(outra.pubkey(), Papel::Industria).unwrap();
    assert_erro(
        amb.comprar_por(&outra, lote, PRECO * 2, ModoRetirada::Terceiro),
        E::TransicaoInvalida,
    );
}

#[test]
fn so_industria_compra_e_lote_de_leilao_nao_e_comprado() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    let transp = amb.transportador.insecure_clone();
    assert_erro(
        amb.comprar_por(&transp, lote, PRECO, ModoRetirada::Terceiro),
        E::PapelIncorreto,
    );

    // Lote em leilão: a compra direta é recusada.
    let leilao = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.criar_lote(2).unwrap();
    amb.anunciar(leilao).unwrap();
    assert!(!amb.lote(leilao).venda_direta);
    assert_erro(
        amb.comprar(leilao, 1_000_000, ModoRetirada::Terceiro),
        E::LoteNaoEhVendaDireta,
    );
}

#[test]
fn mudar_preco_cancelando_e_reanunciando_na_mesma_transacao() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    let coop = amb.cooperativa.insecure_clone();
    let cancelar = amb.ix_coop_lote(
        &coop.pubkey(),
        lote,
        ecol_lote::instruction::CooperativaCancelAnuncio {},
    );
    let reanunciar = amb.ix_coop_lote(
        &coop.pubkey(),
        lote,
        ecol_lote::instruction::CooperativaAnunciarVenda {
            preco_centavos: PRECO * 2,
        },
    );
    amb.enviar_varias(&[cancelar, reanunciar], &[&coop])
        .unwrap();
    let l = amb.lote(lote);
    assert!(l.venda_direta);
    assert_eq!(l.preco_minimo_centavos, PRECO * 2);

    // Retirar da venda: volta a SemLance, sem a marca de venda direta.
    let cancelar = amb.ix_coop_lote(
        &coop.pubkey(),
        lote,
        ecol_lote::instruction::CooperativaCancelAnuncio {},
    );
    amb.enviar(cancelar, &[&coop]).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::SemLance);
    assert!(!l.venda_direta);
}

#[test]
fn retirada_por_terceiro_e_pagamento_confirmado_pela_cooperativa() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    amb.comprar(lote, PRECO, ModoRetirada::Terceiro).unwrap();
    // Retirada por terceiro: a indústria não assina no lugar do transportador.
    let (ind, coop) = (
        amb.industria.insecure_clone(),
        amb.cooperativa.insecure_clone(),
    );
    assert_erro(amb.retirar_com(&ind, &coop, lote), E::PapelIncorreto);
    amb.retirar(lote).unwrap();
    assert_eq!(
        dono_do_asset(&amb.svm, &amb.lote(lote).asset),
        amb.transportador.pubkey()
    );
    // Antes do recebimento não há pagamento a confirmar.
    assert_erro(
        amb.confirmar_pagamento_por(&coop, lote),
        E::TransicaoInvalida,
    );
    amb.receber(lote, PESO_G).unwrap();

    amb.confirmar_pagamento_por(&coop, lote).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Reciclado);
    assert_eq!(l.valor_liberado_centavos, PRECO);
}

#[test]
fn retirada_propria_e_assinada_pela_industria_compradora() {
    let mut amb = Ambiente::novo();
    let lote = lote_a_venda(&mut amb);
    amb.comprar(lote, PRECO, ModoRetirada::Propria).unwrap();
    let (ind, coop, transp) = (
        amb.industria.insecure_clone(),
        amb.cooperativa.insecure_clone(),
        amb.transportador.insecure_clone(),
    );
    // Outra indústria (ou um transportador) não retira no lugar da compradora.
    let outra = Keypair::new();
    amb.registrar(outra.pubkey(), Papel::Industria).unwrap();
    assert_erro(amb.retirar_com(&outra, &coop, lote), E::RetiradaDeOutroModo);
    assert_erro(
        amb.retirar_com(&transp, &coop, lote),
        E::RetiradaDeOutroModo,
    );

    amb.retirar_com(&ind, &coop, lote).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::EmTransporte);
    assert_eq!(l.transportador, ind.pubkey());
    assert_eq!(dono_do_asset(&amb.svm, &l.asset), ind.pubkey());

    amb.receber(lote, PESO_G).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Recebido);
    assert_eq!(dono_do_asset(&amb.svm, &amb.lote(lote).asset), ind.pubkey());
    amb.confirmar_pagamento_por(&coop, lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);
}

#[test]
fn pagamento_so_na_venda_direta_e_pela_cooperativa_do_lote() {
    let mut amb = Ambiente::novo();
    // Lote de leilão recebido: quem libera é o intermediador, não a cooperativa.
    let leilao = amb.lote_ate_recebido(1);
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(
        amb.confirmar_pagamento_por(&coop, leilao),
        E::LoteNaoEhVendaDireta,
    );

    let lote = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.criar_lote(2).unwrap();
    amb.anunciar_venda(lote, PRECO).unwrap();
    amb.comprar(lote, PRECO, ModoRetirada::Terceiro).unwrap();
    amb.retirar(lote).unwrap();
    amb.receber(lote, PESO_G).unwrap();
    let outra = Keypair::new();
    amb.registrar(outra.pubkey(), Papel::Cooperativa).unwrap();
    assert_erro(
        amb.confirmar_pagamento_por(&outra, lote),
        E::NaoEhCooperativaDoLote,
    );
}

#[test]
fn marcar_vendido_fora_e_desfazer() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.set_vendido_fora(lote, true).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::VendidoFora);
    // Marcado como vendido, não vai à venda nem é comprado.
    assert_erro(amb.anunciar_venda(lote, PRECO), E::TransicaoInvalida);
    assert_erro(amb.set_vendido_fora(lote, true), E::TransicaoInvalida);

    amb.set_vendido_fora(lote, false).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::SemLance);
    amb.anunciar_venda(lote, PRECO).unwrap();
    // À venda, não pode ser marcado como vendido fora (retirar da venda antes).
    assert_erro(amb.set_vendido_fora(lote, true), E::TransicaoInvalida);
}
