//! ADR 0011: material com variações (cores do vidro) e garrafas aproximadas.
mod common;

use {common::*, ecol_lote::error::EcolLoteError as E, solana_signer::Signer};

#[test]
fn operador_cadastra_cores_do_vidro() {
    let mut amb = Ambiente::novo();
    amb.preparar_vidro();
    let m = amb.material(VIDRO);
    assert_eq!((m.qtd_variacoes, m.conta_garrafas), (3, true));
    let verde = amb.variacao(VIDRO, VERDE);
    assert_eq!(
        (verde.material, verde.indice, verde.ativa),
        (VIDRO, VERDE, true)
    );
    assert_eq!(&verde.nome[..5], b"Verde");
    amb.atualizar_variacao(VIDRO, VERDE, "Verde-escuro", false)
        .unwrap();
    assert!(!amb.variacao(VIDRO, VERDE).ativa);
}

#[test]
fn lote_de_vidro_exige_cor_ativa() {
    let mut amb = Ambiente::novo();
    amb.preparar_vidro();
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let c = coop.pubkey();

    let sem_cor = amb.dados_entrega_vidro(1, 50_000, 0, 100);
    assert_erro(
        amb.registrar_entrega_com(&c, &coop, false, &balanca, sem_cor, None),
        E::VariacaoObrigatoria,
    );
    amb.atualizar_variacao(VIDRO, MARROM, "Marrom", false)
        .unwrap();
    let marrom = amb.dados_entrega_vidro(1, 50_000, MARROM, 100);
    assert_erro(
        amb.registrar_entrega_com(&c, &coop, false, &balanca, marrom, None),
        E::VariacaoInativa,
    );
    // Plástico não tem variações nem garrafas.
    let mut plastico = amb.dados_entrega(1, 50_000, PLASTICO);
    plastico.qtd_garrafas = 10;
    assert_erro(
        amb.registrar_entrega_com(&c, &coop, false, &balanca, plastico, None),
        E::MaterialSemGarrafas,
    );
    let verde = amb.dados_entrega_vidro(1, 50_000, VERDE, 100);
    amb.registrar_entrega_com(&c, &coop, false, &balanca, verde, None)
        .unwrap();
}

#[test]
fn lote_de_venda_nao_mistura_cores_e_soma_garrafas() {
    let mut amb = Ambiente::novo();
    amb.preparar_vidro();
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let c = coop.pubkey();
    for (id, cor, garrafas) in [(1, VERDE, 100), (2, VERDE, 150), (3, TRANSPARENTE, 80)] {
        let d = amb.dados_entrega_vidro(id, 50_000, cor, garrafas);
        amb.registrar_entrega_com(&c, &coop, false, &balanca, d, None)
            .unwrap();
    }
    let mut dados = amb.dados(1);
    dados.material = VIDRO;
    // Lote de vidro também exige a cor.
    assert_erro(amb.abrir_lote(&coop, dados.clone()), E::VariacaoObrigatoria);
    dados.variacao = VERDE;
    amb.abrir_lote(&coop, dados).unwrap();
    let lote = lote_pda(&c, 1);
    assert_erro(
        amb.add_entregas(&coop, lote, &[entrega_pda(&c, 1), entrega_pda(&c, 3)]),
        E::VariacaoDiferente,
    );
    amb.add_entregas(&coop, lote, &[entrega_pda(&c, 1), entrega_pda(&c, 2)])
        .unwrap();
    let l = amb.lote(lote);
    assert_eq!(
        (l.variacao, l.qtd_garrafas, l.peso_entregas_g),
        (VERDE, 250, 100_000)
    );

    // Desfazer devolve as garrafas.
    amb.desmontar(&coop, lote, &[entrega_pda(&c, 1)]).unwrap();
    assert_eq!(amb.lote(lote).qtd_garrafas, 150);
}

#[test]
fn so_operador_cadastra_variacao() {
    let mut amb = Ambiente::novo();
    amb.criar_material(VIDRO, "Vidro").unwrap();
    let operador = amb.operador.insecure_clone();
    amb.operador = amb.cooperativa.insecure_clone();
    assert_erro(amb.criar_variacao(VIDRO, "Verde"), E::NaoEhOperador);
    amb.operador = operador;
    amb.criar_variacao(VIDRO, "Verde").unwrap();
}
