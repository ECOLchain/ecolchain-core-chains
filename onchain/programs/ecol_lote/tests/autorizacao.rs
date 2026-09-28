//! Quem pode assinar o quê, validações de entrada e modos de operação.
mod common;

use {
    anchor_lang::prelude::Pubkey,
    common::*,
    ecol_lote::{error::EcolLoteError as E, DecisaoDisputa, ModoOperacao, MotivoDisputa, Papel},
    solana_keypair::Keypair,
    solana_signer::Signer,
};

#[test]
fn init_global_exige_upgrade_authority() {
    let mut amb = Ambiente::novo_sem_config();
    let intruso = Keypair::new();
    assert_erro(amb.init_global(&intruso), E::NaoEhUpgradeAuthority);
    let ua = amb.upgrade_authority.insecure_clone();
    amb.init_global(&ua).unwrap();
    assert_eq!(amb.config().admin, amb.admin.pubkey());
    // Não reinicializa.
    assert!(amb.init_global(&ua).is_err());
}

#[test]
fn update_global_so_admin_e_com_limites() {
    let mut amb = Ambiente::novo();
    let intruso = Keypair::new();
    let params = amb.params();
    assert_erro(amb.update_global(&intruso, params.clone()), E::NaoEhAdmin);

    let admin = amb.admin.insecure_clone();
    let mut p = params.clone();
    p.tolerancia_peso_bps = 2_001;
    assert_erro(amb.update_global(&admin, p), E::ParametroInvalido);
    let mut p = params.clone();
    p.prazo_entrega_s = 0;
    assert_erro(amb.update_global(&admin, p), E::ParametroInvalido);

    let mut p = params;
    p.tolerancia_peso_bps = 500;
    amb.update_global(&admin, p).unwrap();
    assert_eq!(amb.config().tolerancia_peso_bps, 500);
}

#[test]
fn cadastro_so_pelo_operador_e_papeis_conferidos() {
    let mut amb = Ambiente::novo();
    let coop2 = Keypair::new();
    // Industria tentando criar lote.
    let ind = amb.industria.insecure_clone();
    assert_erro(amb.criar_lote_por(&ind, 1, PESO_G), E::PapelIncorreto);

    // Carteira sem cadastro: a conta Participante não existe.
    assert!(amb.criar_lote_por(&coop2, 1, PESO_G).is_err());

    amb.registrar(coop2.pubkey(), Papel::Cooperativa).unwrap();
    assert_eq!(amb.participante(&coop2.pubkey()).papel, Papel::Cooperativa);
    amb.set_ativo(coop2.pubkey(), false).unwrap();
    assert_erro(amb.criar_lote_por(&coop2, 1, PESO_G), E::ParticipanteInativo);
    amb.set_ativo(coop2.pubkey(), true).unwrap();
    amb.criar_lote_por(&coop2, 1, PESO_G).unwrap();
}

#[test]
fn validacoes_de_entrada_na_criacao() {
    let mut amb = Ambiente::novo();
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(amb.criar_lote_por(&coop, 1, 0), E::PesoInvalido);
    amb.criar_lote(1).unwrap();
    // Mesmo lote_id da mesma cooperativa: a PDA já existe.
    assert!(amb.criar_lote(1).is_err());
}

#[test]
fn cada_etapa_so_pelo_assinante_certo() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();

    let outra_coop = Keypair::new();
    amb.registrar(outra_coop.pubkey(), Papel::Cooperativa).unwrap();
    assert_erro(amb.anunciar_por(&outra_coop, lote, T0 + 3600, 1), E::NaoEhCooperativaDoLote);
    amb.anunciar(lote).unwrap();

    let operador_falso = Keypair::new();
    amb.set_tempo(T0 + 3600);
    {
        let ix = amb.ix(
            ecol_lote::instruction::OperadorCloseLeilaoSemLance {},
            ecol_lote::accounts::OperadorCloseLeilaoSemLance {
                operador: operador_falso.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        assert_erro(amb.enviar(ix, &[&operador_falso]), E::NaoEhOperador);
    }

    let (ind, inter) = (amb.industria.insecure_clone(), amb.intermediador.insecure_clone());
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(amb.vender_com(&ind, &Keypair::new(), lote, 25_000), E::NaoEhIntermediador);
    // Cooperativa não pode comprar (papel errado).
    assert_erro(amb.vender_com(&coop, &inter, lote, 25_000), E::PapelIncorreto);
    amb.vender(lote).unwrap();

    let transp = amb.transportador.insecure_clone();
    assert_erro(amb.retirar_com(&transp, &outra_coop, lote), E::NaoEhCooperativaDoLote);
    // Transportador tem de ter o papel certo.
    assert_erro(amb.retirar_com(&ind, &coop, lote), E::PapelIncorreto);
    amb.retirar(lote).unwrap();

    let outra_ind = Keypair::new();
    amb.registrar(outra_ind.pubkey(), Papel::Industria).unwrap();
    assert_erro(amb.receber_por(&outra_ind, lote, PESO_G), E::NaoEhIndustriaDoLote);
    assert_erro(amb.receber(lote, 0), E::PesoInvalido);
    amb.receber(lote, PESO_G).unwrap();

    assert_erro(amb.liberar_por(&Keypair::new(), lote), E::NaoEhIntermediador);
    amb.liberar(lote).unwrap();

    assert_erro(amb.agregar_por(&Keypair::new(), lote, Pubkey::new_unique()), E::NaoEhAutoridadeCredito);
}

#[test]
fn estranho_nao_abre_disputa_nem_arbitra() {
    let mut amb = Ambiente::novo();
    let lote = amb.lote_ate_recebido(1);
    let estranho = Keypair::new();
    assert_erro(amb.abrir_disputa(&estranho, lote, MotivoDisputa::Outro), E::NaoEhParteDoLote);

    let transp = amb.transportador.insecure_clone();
    amb.abrir_disputa(&transp, lote, MotivoDisputa::Outro).unwrap();
    assert_erro(
        amb.resolver(&estranho, None, lote, DecisaoDisputa::Prosseguir { novo_peso_g: None }),
        E::NaoEhArbitro,
    );
    let arbitro = amb.arbitro.insecure_clone();
    assert_erro(
        amb.resolver(&arbitro, None, lote, DecisaoDisputa::Prosseguir { novo_peso_g: Some(0) }),
        E::PesoInvalido,
    );
}

#[test]
fn transicoes_fora_de_ordem_sao_rejeitadas() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    assert_erro(amb.vender(lote), E::TransicaoInvalida);
    assert_erro(amb.retirar(lote), E::TransicaoInvalida);
    assert_erro(amb.liberar(lote), E::TransicaoInvalida);
    assert_erro(amb.crank_prazo(lote), E::TransicaoInvalida);
    let ind = amb.industria.insecure_clone();
    // Ainda sem indústria no lote: ela não é parte.
    assert_erro(amb.abrir_disputa(&ind, lote, MotivoDisputa::Outro), E::NaoEhParteDoLote);
    let coop = amb.cooperativa.insecure_clone();
    assert_erro(amb.abrir_disputa(&coop, lote, MotivoDisputa::Outro), E::TransicaoInvalida);
}

#[test]
fn modos_de_operacao() {
    let mut amb = Ambiente::novo();
    let lote = lote_pda(&amb.cooperativa.pubkey(), 1);
    amb.criar_lote(1).unwrap();
    amb.anunciar(lote).unwrap();
    amb.vender(lote).unwrap();

    amb.set_modo(ModoOperacao::SomenteDisputas).unwrap();
    assert_erro(amb.criar_lote(2), E::ProgramaPausado);
    assert_erro(amb.retirar(lote), E::ProgramaPausado);
    // Disputas continuam, para não travar o escrow.
    let coop = amb.cooperativa.insecure_clone();
    amb.abrir_disputa(&coop, lote, MotivoDisputa::FalhaEntrega).unwrap();

    amb.set_modo(ModoOperacao::Pausado).unwrap();
    let arbitro = amb.arbitro.insecure_clone();
    assert_erro(
        amb.resolver(&arbitro, None, lote, DecisaoDisputa::Prosseguir { novo_peso_g: None }),
        E::ProgramaPausado,
    );

    amb.set_modo(ModoOperacao::Normal).unwrap();
    amb.resolver(&arbitro, None, lote, DecisaoDisputa::Prosseguir { novo_peso_g: None }).unwrap();
    amb.retirar(lote).unwrap();
}
