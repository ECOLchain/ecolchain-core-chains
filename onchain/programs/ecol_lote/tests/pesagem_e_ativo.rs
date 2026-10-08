//! F2: atestação Ed25519 da balança e Core asset não transferível.
mod common;

use {
    anchor_lang::{prelude::Pubkey, solana_program::instruction::Instruction},
    common::*,
    ecol_lote::{
        error::EcolLoteError as E,
        pesagem::{mensagem, TipoPesagem},
        EstadoLote, Papel,
    },
    solana_keypair::Keypair,
    solana_signer::Signer,
};

/// `MplCoreError::InvalidAuthority`: nenhum plugin aprovou a operação (asset congelado).
const CORE_INVALID_AUTHORITY: u32 = 9;

#[track_caller]
fn assert_erro_core(
    res: Result<litesvm::types::TransactionMetadata, solana_transaction_error::TransactionError>,
    codigo: u32,
) {
    use solana_transaction::InstructionError;
    use solana_transaction_error::TransactionError;
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, codigo)
        }
        outro => panic!("esperava erro {codigo} do Metaplex Core, veio {outro:?}"),
    }
}

fn lote1(amb: &Ambiente) -> Pubkey {
    lote_pda(&amb.cooperativa.pubkey(), 1)
}

fn tentar(
    amb: &mut Ambiente,
    ixs: Vec<Instruction>,
) -> Result<litesvm::types::TransactionMetadata, solana_transaction_error::TransactionError> {
    let coop = amb.cooperativa.insecure_clone();
    amb.enviar_varias(&ixs, &[&coop])
}

/// Lote 1 em montagem com um lote de origem de `PESO_G`: pronto para o fechamento com pesagem.
fn preparar(amb: &mut Ambiente) {
    let coop = amb.cooperativa.insecure_clone();
    let balanca = amb.balanca_coop.insecure_clone();
    let dados = amb.dados(1);
    amb.abrir_lote(&coop, dados).unwrap();
    amb.registrar_entrega(&coop, &balanca, 1, PESO_G, PLASTICO)
        .unwrap();
    let lote = lote1(amb);
    amb.add_entregas(&coop, lote, &[entrega_pda(&coop.pubkey(), 1)])
        .unwrap();
}

fn ix_criar(amb: &Ambiente, peso_g: u64, ts: i64) -> Instruction {
    let mut dados = amb.dados_fechamento(peso_g);
    dados.ts_pesagem = ts;
    amb.ix_fechar_lote(
        &amb.cooperativa.pubkey(),
        balanca_pda(&amb.balanca_coop.pubkey()),
        lote1(amb),
        dados,
    )
}

#[test]
fn pesagem_valida_cria_lote_e_asset_congelado() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, T0);
    let criar = ix_criar(&amb, PESO_G, T0);
    tentar(&mut amb, vec![pesagem, criar]).unwrap();
    assert_eq!(amb.lote(lote).estado, EstadoLote::Criado);
    assert_eq!(
        dono_do_asset(&amb.svm, &asset_pda(&lote)),
        amb.cooperativa.pubkey()
    );
}

#[test]
fn sem_instrucao_ed25519() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(tentar(&mut amb, vec![criar]), E::AtestacaoAusente);
}

#[test]
fn ed25519_precisa_ser_a_instrucao_imediatamente_anterior() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, T0);
    // Uma instrução qualquer do próprio programa entre a assinatura e a criação.
    let intermediaria = amb.ix(
        ecol_lote::instruction::OperadorSetParticipanteAtivo { ativo: true },
        ecol_lote::accounts::OperadorSetParticipanteAtivo {
            operador: amb.operador.pubkey(),
            config: config_pda(),
            participante: participante_pda(&amb.cooperativa.pubkey()),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    let criar = ix_criar(&amb, PESO_G, T0);
    let (coop, op) = (
        amb.cooperativa.insecure_clone(),
        amb.operador.insecure_clone(),
    );
    assert_erro(
        amb.enviar_varias(&[pesagem, intermediaria, criar], &[&coop, &op]),
        E::AtestacaoAusente,
    );
}

#[test]
fn assinatura_de_outra_chave() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let falsa = Keypair::new();
    let pesagem = ix_pesagem(&falsa, TipoPesagem::Origem, &lote, PESO_G, T0);
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::BalancaIncorreta);
}

#[test]
fn peso_declarado_diferente_do_assinado() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, T0);
    let criar = ix_criar(&amb, PESO_G * 2, T0);
    assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::PesagemDivergente);
}

#[test]
fn assinatura_de_outro_lote_nao_serve() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let outro_lote = lote_pda(&amb.cooperativa.pubkey(), 2);
    let pesagem = ix_pesagem(
        &amb.balanca_coop,
        TipoPesagem::Origem,
        &outro_lote,
        PESO_G,
        T0,
    );
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::PesagemDivergente);
}

#[test]
fn assinatura_de_recebimento_nao_cria_lote() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let pesagem = ix_pesagem(
        &amb.balanca_coop,
        TipoPesagem::Recebimento,
        &lote,
        PESO_G,
        T0,
    );
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::PesagemDivergente);
}

#[test]
fn pesagem_antiga_ou_futura() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    for ts in [T0 - 15 * 60 - 1, T0 + 15 * 60 + 1] {
        let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, ts);
        let criar = ix_criar(&amb, PESO_G, ts);
        assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::PesagemExpirada);
    }
    // Na borda da janela ainda vale.
    let ts = T0 - 15 * 60;
    let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, ts);
    let criar = ix_criar(&amb, PESO_G, ts);
    tentar(&mut amb, vec![pesagem, criar]).unwrap();
}

/// Instrução Ed25519 válida para o runtime, mas que aponta os dados por índice explícito
/// em vez de `u16::MAX`. É o padrão que permite verificar bytes de outra instrução.
#[test]
fn ed25519_com_indice_explicito_e_rejeitada() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let msg = mensagem(TipoPesagem::Origem, &lote, PESO_G, T0);
    let sig = amb.balanca_coop.sign_message(&msg);
    let (pk_off, sig_off, msg_off) = (16u16, 48u16, 112u16);
    let mut data = vec![1u8, 0];
    for v in [sig_off, 0, pk_off, 0, msg_off, msg.len() as u16, 0] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    data.extend_from_slice(&amb.balanca_coop.pubkey().to_bytes());
    data.extend_from_slice(sig.as_ref());
    data.extend_from_slice(&msg);
    let pesagem = Instruction {
        program_id: solana_sdk_ids::ed25519_program::ID,
        accounts: vec![],
        data,
    };
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::AtestacaoInvalida);
}

#[test]
fn balanca_inativa_ou_de_outro_dono() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    let bal = balanca_pda(&amb.balanca_coop.pubkey());

    amb.set_balanca_ativa(bal, false).unwrap();
    let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, T0);
    let criar = ix_criar(&amb, PESO_G, T0);
    assert_erro(
        tentar(&mut amb, vec![pesagem.clone(), criar.clone()]),
        E::BalancaInativa,
    );
    amb.set_balanca_ativa(bal, true).unwrap();

    // Cooperativa usando a balança da indústria.
    let pesagem = ix_pesagem(&amb.balanca_ind, TipoPesagem::Origem, &lote, PESO_G, T0);
    let dados = amb.dados_fechamento(PESO_G);
    let criar = amb.ix_fechar_lote(
        &amb.cooperativa.pubkey(),
        balanca_pda(&amb.balanca_ind.pubkey()),
        lote,
        dados,
    );
    assert_erro(
        tentar(&mut amb, vec![pesagem, criar]),
        E::BalancaDeOutroDono,
    );
}

#[test]
fn equipamento_nao_pode_ter_dois_donos() {
    let mut amb = Ambiente::novo();
    let coop2 = Keypair::new();
    amb.registrar(coop2.pubkey(), Papel::Cooperativa).unwrap();
    let dispositivo = amb.balanca_coop.pubkey();
    assert!(amb.registrar_balanca(coop2.pubkey(), dispositivo).is_err());
    // Transportador não tem balança.
    let transp = amb.transportador.pubkey();
    assert_erro(
        amb.registrar_balanca(transp, Keypair::new().pubkey()),
        E::PapelIncorreto,
    );
}

#[test]
fn uri_invalida() {
    let mut amb = Ambiente::novo();
    preparar(&mut amb);
    let lote = lote1(&amb);
    for uri in [String::new(), "x".repeat(201)] {
        let mut dados = amb.dados_fechamento(PESO_G);
        dados.uri = uri;
        let pesagem = ix_pesagem(&amb.balanca_coop, TipoPesagem::Origem, &lote, PESO_G, T0);
        let criar = amb.ix_fechar_lote(
            &amb.cooperativa.pubkey(),
            balanca_pda(&amb.balanca_coop.pubkey()),
            lote,
            dados,
        );
        assert_erro(tentar(&mut amb, vec![pesagem, criar]), E::UriInvalida);
    }
}

#[test]
fn dono_nao_transfere_o_asset_por_fora() {
    let mut amb = Ambiente::novo();
    amb.criar_lote(1).unwrap();
    let lote = lote1(&amb);
    let asset = asset_pda(&lote);
    let comprador_paralelo = Keypair::new().pubkey();
    let ix = mpl_core::instructions::TransferV1Builder::new()
        .asset(asset)
        .payer(amb.patrocinador.pubkey())
        .authority(Some(amb.cooperativa.pubkey()))
        .new_owner(comprador_paralelo)
        .instruction();
    let coop = amb.cooperativa.insecure_clone();
    assert_erro_core(amb.enviar(ix, &[&coop]), CORE_INVALID_AUTHORITY);
    assert_eq!(dono_do_asset(&amb.svm, &asset), amb.cooperativa.pubkey());

    let burn = mpl_core::instructions::BurnV1Builder::new()
        .asset(asset)
        .payer(amb.patrocinador.pubkey())
        .authority(Some(amb.cooperativa.pubkey()))
        .instruction();
    assert_erro_core(amb.enviar(burn, &[&coop]), CORE_INVALID_AUTHORITY);
    assert_eq!(dono_do_asset(&amb.svm, &asset), amb.cooperativa.pubkey());

    // Controle: o mesmo builder transfere um asset Core comum (sem plugins) do mesmo dono.
    // Garante que a recusa acima vem do congelamento, não de instrução mal montada.
    let livre = Keypair::new();
    let criar = mpl_core::instructions::CreateV2Builder::new()
        .asset(livre.pubkey())
        .payer(amb.patrocinador.pubkey())
        .owner(Some(amb.cooperativa.pubkey()))
        .name("controle".to_string())
        .uri("https://example.com".to_string())
        .instruction();
    amb.enviar(criar, &[&livre]).unwrap();
    let ix = mpl_core::instructions::TransferV1Builder::new()
        .asset(livre.pubkey())
        .payer(amb.patrocinador.pubkey())
        .authority(Some(amb.cooperativa.pubkey()))
        .new_owner(comprador_paralelo)
        .instruction();
    amb.enviar(ix, &[&coop]).unwrap();
    assert_eq!(dono_do_asset(&amb.svm, &livre.pubkey()), comprador_paralelo);
}

#[test]
fn asset_de_outro_lote_e_rejeitado() {
    let mut amb = Ambiente::novo();
    // Lote 2 fica parado em `Criado`; o lote 3 é vendido e sua retirada recebe o asset do lote 2.
    amb.criar_lote(2).unwrap();
    let lote2 = lote_pda(&amb.cooperativa.pubkey(), 2);
    amb.criar_lote(3).unwrap();
    let lote3 = lote_pda(&amb.cooperativa.pubkey(), 3);
    amb.anunciar(lote3).unwrap();
    amb.vender(lote3).unwrap();
    let mut ix = amb.ix(
        ecol_lote::instruction::TransportadorPickupLote {},
        ecol_lote::accounts::TransportadorPickupLote {
            transportador: amb.transportador.pubkey(),
            transportador_assinante: amb.transportador.pubkey(),
            transportador_carteira: None,
            transportador_part: participante_pda(&amb.transportador.pubkey()),
            cooperativa: amb.cooperativa.pubkey(),
            cooperativa_assinante: amb.cooperativa.pubkey(),
            cooperativa_carteira: None,
            cooperativa_part: participante_pda(&amb.cooperativa.pubkey()),
            config: config_pda(),
            lote: lote3,
            custodia: amb.custodia(&lote3),
            event_authority: event_authority(),
            program: ecol_lote::id(),
        },
    );
    let asset3 = asset_pda(&lote3);
    let asset2 = asset_pda(&lote2);
    for m in ix.accounts.iter_mut() {
        if m.pubkey == asset3 {
            m.pubkey = asset2;
        }
    }
    let (t, c) = (
        amb.transportador.insecure_clone(),
        amb.cooperativa.insecure_clone(),
    );
    assert_erro(amb.enviar(ix, &[&t, &c]), E::AssetIncorreto);
}

#[test]
fn recebimento_com_pesagem_atestada_da_industria() {
    let mut amb = Ambiente::novo();
    let lote = lote1(&amb);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();
    amb.retirar(lote).unwrap();

    let ind = amb.industria.insecure_clone();
    let bal_ind = balanca_pda(&amb.balanca_ind.pubkey());
    let bal_coop = balanca_pda(&amb.balanca_coop.pubkey());

    // Balança sem timestamp (e vice-versa) é rejeitado.
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, Some(bal_ind), None);
    assert_erro(amb.enviar(ix, &[&ind]), E::AtestacaoAusente);
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, None, Some(T0));
    assert_erro(amb.enviar(ix, &[&ind]), E::AtestacaoAusente);

    // Indústria não pode usar a balança da cooperativa.
    let pesagem = ix_pesagem(
        &amb.balanca_coop,
        TipoPesagem::Recebimento,
        &lote,
        PESO_G,
        T0,
    );
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, Some(bal_coop), Some(T0));
    assert_erro(
        amb.enviar_varias(&[pesagem, ix], &[&ind]),
        E::BalancaDeOutroDono,
    );

    // Assinatura de origem não vale como recebimento.
    let pesagem = ix_pesagem(&amb.balanca_ind, TipoPesagem::Origem, &lote, PESO_G, T0);
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, Some(bal_ind), Some(T0));
    assert_erro(
        amb.enviar_varias(&[pesagem, ix], &[&ind]),
        E::PesagemDivergente,
    );

    let pesagem = ix_pesagem(
        &amb.balanca_ind,
        TipoPesagem::Recebimento,
        &lote,
        PESO_G,
        T0,
    );
    let ix = amb.ix_receber(&ind.pubkey(), lote, PESO_G, Some(bal_ind), Some(T0));
    amb.enviar_varias(&[pesagem, ix], &[&ind]).unwrap();
    let l = amb.lote(lote);
    assert_eq!(l.estado, EstadoLote::Recebido);
    assert!(l.recebimento_atestado);
    assert_eq!(dono_do_asset(&amb.svm, &asset_pda(&lote)), ind.pubkey());
}
