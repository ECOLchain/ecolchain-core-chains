//! ADR 0011: várias carteiras por participante, cada uma com nome, todas assinando pelo ator.
mod common;

use {
    common::*,
    ecol_lote::{error::EcolLoteError as E, EstadoLote, Papel},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn titular_e_operador_vinculam_carteiras_com_nome() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    let c = coop.pubkey();
    let balcao = Keypair::new();
    amb.add_carteira_por(&coop, c, balcao.pubkey(), "Balcão")
        .unwrap();
    let cart = amb.carteira(&balcao.pubkey());
    assert_eq!((cart.participante, cart.ativa), (c, true));
    assert_eq!(&cart.nome[..7], "Balcão".as_bytes());

    // O operador também vincula, inclusive o registro do próprio titular (só para nomeá-lo).
    let operador = amb.operador.insecure_clone();
    amb.add_carteira_por(&operador, c, c, "Diretoria").unwrap();
    amb.update_carteira_por(&coop, c, balcao.pubkey(), "Balcão 2", false)
        .unwrap();
    assert!(!amb.carteira(&balcao.pubkey()).ativa);

    // Terceiros, e a própria carteira vinculada, não gerenciam.
    let intruso = Keypair::new();
    assert_erro(
        amb.add_carteira_por(&intruso, c, Keypair::new().pubkey(), "X"),
        E::NaoEhOperador,
    );
    assert_erro(
        amb.update_carteira_por(&balcao, c, balcao.pubkey(), "X", true),
        E::NaoEhOperador,
    );
    // Nome validado.
    assert_erro(
        amb.add_carteira_por(&coop, c, Keypair::new().pubkey(), ""),
        E::NomeInvalido,
    );
}

#[test]
fn carteira_pertence_a_um_so_ator() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    let c = coop.pubkey();
    let ind = amb.industria.insecure_clone();
    // Titular de outro cadastro não vira carteira vinculada.
    assert_erro(
        amb.add_carteira_por(&coop, c, ind.pubkey(), "Indústria"),
        E::CarteiraEmUso,
    );
    // Carteira já vinculada não é vinculada de novo (a PDA já existe) nem vira titular.
    let k = Keypair::new();
    amb.add_carteira_por(&coop, c, k.pubkey(), "Balcão")
        .unwrap();
    assert!(amb
        .add_carteira_por(&ind, ind.pubkey(), k.pubkey(), "Outra")
        .is_err());
    assert_erro(amb.registrar(k.pubkey(), Papel::Coletor), E::CarteiraEmUso);
    // Chave de papel do config não é vinculada.
    let operador = amb.operador.pubkey();
    assert_erro(
        amb.add_carteira_por(&coop, c, operador, "Operador"),
        E::CarteiraReservada,
    );
}

#[test]
fn carteira_vinculada_assina_pelo_ator() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let c = coop.pubkey();
    let balcao = Keypair::new();
    amb.add_carteira_por(&coop, c, balcao.pubkey(), "Balcão")
        .unwrap();

    let dados = amb.dados_entrega(1, PESO_G, PLASTICO);
    amb.registrar_entrega_com(&c, &balcao, true, &balanca, dados, None)
        .unwrap();
    // A entrega é do titular: a PDA e o campo usam a carteira titular.
    assert_eq!(amb.entrega(&c, 1).cooperativa, c);

    // Sem a conta da carteira, ou com ela desativada, a assinatura não vale.
    let dados = amb.dados_entrega(2, PESO_G, PLASTICO);
    assert_erro(
        amb.registrar_entrega_com(&c, &balcao, false, &balanca, dados.clone(), None),
        E::CarteiraNaoAutorizada,
    );
    amb.update_carteira_por(&coop, c, balcao.pubkey(), "Balcão", false)
        .unwrap();
    assert_erro(
        amb.registrar_entrega_com(&c, &balcao, true, &balanca, dados, None),
        E::CarteiraNaoAutorizada,
    );
}

#[test]
fn carteira_de_outro_ator_nao_assina() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let ind = amb.industria.insecure_clone();
    let k = Keypair::new();
    amb.add_carteira_por(&ind, ind.pubkey(), k.pubkey(), "Compras")
        .unwrap();
    let dados = amb.dados_entrega(1, PESO_G, PLASTICO);
    assert_erro(
        amb.registrar_entrega_com(&coop.pubkey(), &k, true, &balanca, dados, None),
        E::CarteiraNaoAutorizada,
    );
}

#[test]
fn importador_usa_carteira_vinculada() {
    let mut amb = Ambiente::novo();
    amb.preparar_vidro();
    let imp = Keypair::new();
    amb.registrar(imp.pubkey(), Papel::Importador).unwrap();
    let loja = Keypair::new();
    amb.add_carteira_por(&imp, imp.pubkey(), loja.pubkey(), "Loja Centro")
        .unwrap();
    let dados = ecol_lote::DadosDistribuicao {
        distrib_id: 1,
        material: VIDRO,
        variacao: VERDE,
        qtd_garrafas: 1_200,
        peso_estimado_g: 600_000,
        doc_hash: h("NF 1"),
        emitido_em: T0,
    };
    amb.criar_distribuicao_por(&imp.pubkey(), &loja, true, dados)
        .unwrap();
    assert_eq!(amb.distribuicao(&imp.pubkey(), 1).importador, imp.pubkey());
}

#[test]
fn retirada_e_recebimento_com_carteiras_vinculadas() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();

    let (coop, transp, ind) = (
        amb.cooperativa.insecure_clone(),
        amb.transportador.insecure_clone(),
        amb.industria.insecure_clone(),
    );
    let (patio, motorista, portaria) = (Keypair::new(), Keypair::new(), Keypair::new());
    amb.add_carteira_por(&coop, coop.pubkey(), patio.pubkey(), "Pátio")
        .unwrap();
    amb.add_carteira_por(&transp, transp.pubkey(), motorista.pubkey(), "Motorista")
        .unwrap();
    amb.add_carteira_por(&ind, ind.pubkey(), portaria.pubkey(), "Portaria")
        .unwrap();

    let ix = amb.ix(
        ecol_lote::instruction::TransportadorPickupLote { mtr: 123_456 },
        ecol_lote::accounts::TransportadorPickupLote {
            transportador: transp.pubkey(),
            transportador_assinante: motorista.pubkey(),
            transportador_carteira: Some(carteira_pda(&motorista.pubkey())),
            transportador_part: participante_pda(&transp.pubkey()),
            cooperativa: coop.pubkey(),
            cooperativa_assinante: patio.pubkey(),
            cooperativa_carteira: Some(carteira_pda(&patio.pubkey())),
            cooperativa_part: participante_pda(&coop.pubkey()),
            config: config_pda(),
            lote,
            custodia: amb.custodia(&lote),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[&motorista, &patio]).unwrap();
    // A custódia vai para o titular, não para a carteira que assinou.
    let l = amb.lote(lote);
    assert_eq!(l.transportador, transp.pubkey());
    assert_eq!(dono_do_asset(&amb.svm, &l.asset), transp.pubkey());

    let ix = amb.ix(
        ecol_lote::instruction::IndustriaConfirmRecebimento {
            peso_recebido_g: PESO_G,
            ts_pesagem: None,
        },
        ecol_lote::accounts::IndustriaConfirmRecebimento {
            industria: ind.pubkey(),
            industria_assinante: portaria.pubkey(),
            industria_carteira: Some(carteira_pda(&portaria.pubkey())),
            balanca: None,
            instrucoes_sysvar: solana_instructions_sysvar::ID,
            config: config_pda(),
            lote,
            custodia: amb.custodia(&lote),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    amb.enviar(ix, &[&portaria]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Recebido);
}
