//! ADR 0011: logística reversa do importador, Clean Tech no lugar da cooperativa e vidro com cor e garrafas.
mod common;

use {
    anchor_lang::error::ErrorCode as AnchorErro,
    common::*,
    ecol_lote::{
        error::EcolLoteError as E, DadosColeta, DadosDistribuicao, EstadoColeta, EstadoLote,
        OrigemEntrega, Papel,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

const PESO_COLETA_G: u64 = 300_000; // 300 kg

struct Atores {
    importador: Keypair,
    cleantech: Keypair,
    balanca_ct: Keypair,
}

/// Ambiente padrão + vidro, um importador e uma Clean Tech com balança.
fn preparar() -> (Ambiente, Atores) {
    let mut amb = Ambiente::novo();
    amb.preparar_vidro();
    let a = Atores {
        importador: Keypair::new(),
        cleantech: Keypair::new(),
        balanca_ct: Keypair::new(),
    };
    amb.registrar(a.importador.pubkey(), Papel::Importador)
        .unwrap();
    amb.registrar(a.cleantech.pubkey(), Papel::CleanTech)
        .unwrap();
    amb.registrar_balanca(a.cleantech.pubkey(), a.balanca_ct.pubkey())
        .unwrap();
    (amb, a)
}

fn dados_coleta(id: u64, cor: u8, garrafas: u32) -> DadosColeta {
    DadosColeta {
        coleta_id: id,
        material: VIDRO,
        variacao: cor,
        qtd_garrafas: garrafas,
        peso_estimado_g: PESO_COLETA_G,
        local_ref: h("ponto-de-coleta+salt"),
        coletado_em: T0 - 86_400,
    }
}

/// Coleta `id` registrada e enviada à Clean Tech.
fn coleta_enviada(amb: &mut Ambiente, a: &Atores, id: u64) {
    let imp = a.importador.pubkey();
    amb.registrar_coleta_por(&imp, &a.importador, dados_coleta(id, VERDE, 600))
        .unwrap();
    amb.entregar_coleta(&a.importador, id, a.cleantech.pubkey())
        .unwrap();
}

/// A Clean Tech pesa a coleta e registra o lote de origem `entrega_id`.
fn receber_coleta(
    amb: &mut Ambiente,
    a: &Atores,
    entrega_id: u64,
    coleta_id: u64,
) -> Result<litesvm::types::TransactionMetadata, solana_transaction_error::TransactionError> {
    let coleta = coleta_pda(&a.importador.pubkey(), coleta_id);
    let mut dados = amb.dados_entrega_vidro(entrega_id, PESO_COLETA_G, VERDE, 590);
    dados.origem = OrigemEntrega::Importador;
    dados.origem_ref = coleta.to_bytes();
    let ct = a.cleantech.pubkey();
    amb.registrar_entrega_com(&ct, &a.cleantech, false, &a.balanca_ct, dados, Some(coleta))
}

#[test]
fn importador_registra_garrafas_distribuidas() {
    let (mut amb, a) = preparar();
    let imp = a.importador.pubkey();
    let dados = DadosDistribuicao {
        distrib_id: 1,
        material: VIDRO,
        variacao: MARROM,
        qtd_garrafas: 12_000,
        peso_estimado_g: 6_000_000,
        doc_hash: h("DI 26/0001234-5"),
        emitido_em: T0 - 30 * 86_400,
    };
    amb.criar_distribuicao_por(&imp, &a.importador, false, dados.clone())
        .unwrap();
    let d = amb.distribuicao(&imp, 1);
    assert_eq!(
        (d.importador, d.variacao, d.qtd_garrafas),
        (imp, MARROM, 12_000)
    );

    // Só importador registra distribuição; garrafas são obrigatórias e o material tem de contá-las.
    let coop = amb.cooperativa.insecure_clone();
    let mut outra = dados.clone();
    outra.distrib_id = 2;
    assert_erro(
        amb.criar_distribuicao_por(&coop.pubkey(), &coop, false, outra.clone()),
        E::PapelIncorreto,
    );
    outra.qtd_garrafas = 0;
    assert_erro(
        amb.criar_distribuicao_por(&imp, &a.importador, false, outra.clone()),
        E::GarrafasInvalidas,
    );
    outra.qtd_garrafas = 10;
    // Data de emissão no futuro.
    let mut futura = outra.clone();
    futura.emitido_em = T0 + 60;
    assert_erro(
        amb.criar_distribuicao_por(&imp, &a.importador, false, futura),
        E::DataInvalida,
    );
    outra.material = PLASTICO;
    outra.variacao = 0;
    assert_erro(
        amb.criar_distribuicao_por(&imp, &a.importador, false, outra),
        E::MaterialSemGarrafas,
    );
}

#[test]
fn coleta_do_importador_ate_a_nf_de_reciclagem() {
    let (mut amb, a) = preparar();
    let imp = a.importador.pubkey();
    let ct = a.cleantech.pubkey();
    coleta_enviada(&mut amb, &a, 1);
    assert_eq!(amb.coleta(&imp, 1).estado, EstadoColeta::EmEntrega);

    receber_coleta(&mut amb, &a, 10, 1).unwrap();
    let c = amb.coleta(&imp, 1);
    let entrega = entrega_pda(&ct, 10);
    assert_eq!((c.estado, c.entrega), (EstadoColeta::Entregue, entrega));
    let e = amb.entrega(&ct, 10);
    assert_eq!(
        (e.origem, e.variacao, e.qtd_garrafas),
        (OrigemEntrega::Importador, VERDE, 590)
    );
    // A mesma coleta não é recebida duas vezes.
    assert_erro(receber_coleta(&mut amb, &a, 11, 1), E::ColetaEstadoInvalido);

    // A Clean Tech monta, fecha, vende e entrega o lote de venda, como uma cooperativa.
    let mut dados = amb.dados(1);
    dados.material = VIDRO;
    dados.variacao = VERDE;
    amb.abrir_lote(&a.cleantech, dados).unwrap();
    let lote = lote_pda(&ct, 1);
    amb.add_entregas(&a.cleantech, lote, &[entrega]).unwrap();
    assert_eq!(amb.lote(lote).qtd_garrafas, 590);
    amb.fechar_lote(&a.cleantech, &a.balanca_ct, lote, PESO_COLETA_G)
        .unwrap();
    amb.anunciar_por(&a.cleantech, lote, T0 + 3600, 10_000)
        .unwrap();

    // NF antes da reciclagem é recusada.
    assert_erro(
        amb.informar_reciclagem(&a.importador, 1, entrega, lote, h("NF 123")),
        E::LoteNaoReciclado,
    );

    amb.vender(lote).unwrap();
    let transp = amb.transportador.insecure_clone();
    amb.retirar_com(&transp, &a.cleantech, lote).unwrap();
    amb.receber(lote, PESO_COLETA_G).unwrap();
    amb.liberar(lote).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Reciclado);

    // Entrega e lote têm de ser os da coleta.
    assert_erro_anchor(
        amb.informar_reciclagem(&a.importador, 1, entrega, lote_pda(&ct, 9), h("NF")),
        AnchorErro::AccountNotInitialized,
    );
    assert_erro_anchor(
        amb.informar_reciclagem(&a.importador, 1, lote, lote, h("NF")),
        AnchorErro::AccountDiscriminatorMismatch,
    );
    assert_erro(
        amb.informar_reciclagem(&a.importador, 1, entrega, lote, [0; 32]),
        E::ReferenciaVazia,
    );
    amb.informar_reciclagem(&a.importador, 1, entrega, lote, h("NF 4567"))
        .unwrap();
    let c = amb.coleta(&imp, 1);
    assert_eq!(
        (c.estado, c.nf_reciclagem_hash),
        (EstadoColeta::Reciclada, h("NF 4567"))
    );
}

#[test]
fn coleta_so_entra_no_destino_escolhido() {
    let (mut amb, a) = preparar();
    coleta_enviada(&mut amb, &a, 1);
    // A cooperativa (não é o destino) tenta registrar a coleta enviada à Clean Tech.
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let coleta = coleta_pda(&a.importador.pubkey(), 1);
    let mut dados = amb.dados_entrega_vidro(20, PESO_COLETA_G, VERDE, 600);
    dados.origem = OrigemEntrega::Importador;
    dados.origem_ref = coleta.to_bytes();
    assert_erro(
        amb.registrar_entrega_com(
            &coop.pubkey(),
            &coop,
            false,
            &balanca,
            dados.clone(),
            Some(coleta),
        ),
        E::ColetaDeOutroDestino,
    );
    // Origem Importador sem a coleta, ou coleta com outra origem.
    assert_erro(
        amb.registrar_entrega_com(&coop.pubkey(), &coop, false, &balanca, dados.clone(), None),
        E::ColetaAusente,
    );
    let mut doacao = dados.clone();
    doacao.origem = OrigemEntrega::Doacao;
    assert_erro(
        amb.registrar_entrega_com(&coop.pubkey(), &coop, false, &balanca, doacao, Some(coleta)),
        E::ColetaAusente,
    );
    // Cor diferente da coleta.
    let ct = a.cleantech.pubkey();
    let mut marrom = dados;
    marrom.variacao = MARROM;
    assert_erro(
        amb.registrar_entrega_com(
            &ct,
            &a.cleantech,
            false,
            &a.balanca_ct,
            marrom,
            Some(coleta),
        ),
        E::ColetaDivergente,
    );
}

#[test]
fn importador_cancela_envio_e_reenvia() {
    let (mut amb, a) = preparar();
    let imp = a.importador.pubkey();
    coleta_enviada(&mut amb, &a, 1);
    amb.cancelar_entrega_coleta(&a.importador, 1).unwrap();
    let c = amb.coleta(&imp, 1);
    assert_eq!(
        (c.estado, c.destino),
        (EstadoColeta::Coletada, Default::default())
    );
    assert_erro(
        amb.cancelar_entrega_coleta(&a.importador, 1),
        E::ColetaEstadoInvalido,
    );
    // Destino precisa ser cooperativa ou Clean Tech.
    let industria = amb.industria.pubkey();
    assert_erro(
        amb.entregar_coleta(&a.importador, 1, industria),
        E::PapelIncorreto,
    );
    let coop = amb.cooperativa.pubkey();
    amb.entregar_coleta(&a.importador, 1, coop).unwrap();
    assert_eq!(amb.coleta(&imp, 1).destino, coop);
}

#[test]
fn coleta_de_outro_importador_nao_e_movida() {
    let (mut amb, a) = preparar();
    coleta_enviada(&mut amb, &a, 1);
    let outro = Keypair::new();
    amb.registrar(outro.pubkey(), Papel::Importador).unwrap();
    // As seeds da coleta usam o importador informado: a do primeiro não existe para o segundo.
    assert_erro_anchor(
        amb.cancelar_entrega_coleta(&outro, 1),
        AnchorErro::AccountNotInitialized,
    );
}

#[test]
fn importador_nao_monta_lote_de_venda() {
    let (mut amb, a) = preparar();
    let mut dados = amb.dados(1);
    dados.material = VIDRO;
    dados.variacao = VERDE;
    assert_erro(amb.abrir_lote(&a.importador, dados), E::PapelIncorreto);
}
