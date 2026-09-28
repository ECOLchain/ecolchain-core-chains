//! Ambiente LiteSVM compartilhado pelos testes do `ecol_lote`.
#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    ecol_lote::{
        accounts as ctas, instruction as ixs, ConfigParams, DecisaoDisputa, EstadoLote,
        GlobalConfig, Lote, Material, ModoOperacao, MotivoDisputa, Papel, Participante,
        SEED_GLOBAL, SEED_LOTE, SEED_PARTICIPANTE,
    },
    litesvm::{types::TransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError},
    solana_transaction_error::TransactionError,
};

pub const PRAZO_ENTREGA_S: i64 = 7 * 24 * 3600;
pub const TOLERANCIA_BPS: u16 = 200; // 2%
pub const T0: i64 = 1_790_000_000;
pub const PESO_G: u64 = 1_000_000; // 1 t

pub struct Ambiente {
    pub svm: LiteSVM,
    pub upgrade_authority: Keypair,
    pub admin: Keypair,
    pub arbitro: Keypair,
    pub operador: Keypair,
    pub intermediador: Keypair,
    pub credito_autoridade: Keypair,
    pub cooperativa: Keypair,
    pub industria: Keypair,
    pub transportador: Keypair,
}

pub fn pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &ecol_lote::id()).0
}

pub fn config_pda() -> Pubkey {
    pda(&[SEED_GLOBAL])
}

pub fn participante_pda(carteira: &Pubkey) -> Pubkey {
    pda(&[SEED_PARTICIPANTE, carteira.as_ref()])
}

pub fn lote_pda(cooperativa: &Pubkey, lote_id: u64) -> Pubkey {
    pda(&[SEED_LOTE, cooperativa.as_ref(), &lote_id.to_le_bytes()])
}

pub fn event_authority() -> Pubkey {
    pda(&[b"__event_authority"])
}

pub fn h(tag: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    let b = tag.as_bytes();
    out[..b.len().min(32)].copy_from_slice(&b[..b.len().min(32)]);
    out
}

/// Código numérico de um erro do programa (6000 + índice).
pub fn codigo(e: ecol_lote::error::EcolLoteError) -> u32 {
    e.into()
}

impl Ambiente {
    /// SVM com o programa carregado e a upgrade authority gravada no ProgramData
    /// (o LiteSVM a cria vazia), sem inicializar a configuração.
    pub fn novo_sem_config() -> Self {
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/ecol_lote.so"));
        svm.add_program(ecol_lote::id(), bytes).unwrap();

        let upgrade_authority = Keypair::new();
        let program_data = Pubkey::find_program_address(
            &[ecol_lote::id().as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        )
        .0;
        let mut conta = svm.get_account(&program_data).unwrap();
        // Layout ProgramData: tag u32 | slot u64 | Option<Pubkey> (1 + 32).
        conta.data[12] = 1;
        conta.data[13..45].copy_from_slice(upgrade_authority.pubkey().as_ref());
        svm.set_account(program_data, conta).unwrap();

        let mut amb = Ambiente {
            svm,
            upgrade_authority,
            admin: Keypair::new(),
            arbitro: Keypair::new(),
            operador: Keypair::new(),
            intermediador: Keypair::new(),
            credito_autoridade: Keypair::new(),
            cooperativa: Keypair::new(),
            industria: Keypair::new(),
            transportador: Keypair::new(),
        };
        // Só o operador (fee payer da EcolChain) e o cranker precisam de SOL.
        let operador = amb.operador.pubkey();
        amb.svm.airdrop(&operador, 10_000_000_000).unwrap();
        amb.set_tempo(T0);
        amb
    }

    /// Configuração inicializada e cooperativa, indústria e transportador cadastrados.
    pub fn novo() -> Self {
        let mut amb = Self::novo_sem_config();
        let ua = amb.upgrade_authority.insecure_clone();
        amb.init_global(&ua).unwrap();
        for (carteira, papel) in [
            (amb.cooperativa.pubkey(), Papel::Cooperativa),
            (amb.industria.pubkey(), Papel::Industria),
            (amb.transportador.pubkey(), Papel::Transportador),
        ] {
            amb.registrar(carteira, papel).unwrap();
        }
        amb
    }

    pub fn set_tempo(&mut self, unix_timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    /// Envia com o operador como fee payer, como a EcolChain fará em produção.
    pub fn enviar(&mut self, ix: Instruction, assinantes: &[&Keypair]) -> Result<TransactionMetadata, TransactionError> {
        self.svm.expire_blockhash();
        let payer = self.operador.insecure_clone();
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
        let mut todos: Vec<&Keypair> = vec![&payer];
        for a in assinantes {
            if a.pubkey() != payer.pubkey() {
                todos.push(a);
            }
        }
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &todos).unwrap();
        self.svm.send_transaction(tx).map_err(|f| f.err)
    }

    pub fn ix(&self, data: impl InstructionData, contas: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(ecol_lote::id(), &data.data(), contas.to_account_metas(None))
    }

    pub fn params(&self) -> ConfigParams {
        ConfigParams {
            admin: self.admin.pubkey(),
            arbitro: self.arbitro.pubkey(),
            operador: self.operador.pubkey(),
            intermediador: self.intermediador.pubkey(),
            credito_autoridade: self.credito_autoridade.pubkey(),
            prazo_entrega_s: PRAZO_ENTREGA_S,
            tolerancia_peso_bps: TOLERANCIA_BPS,
        }
    }

    pub fn init_global(&mut self, autoridade: &Keypair) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitGlobal { params: self.params() },
            ctas::AdminInitGlobal {
                payer: self.operador.pubkey(),
                upgrade_authority: autoridade.pubkey(),
                config: config_pda(),
                este_programa: ecol_lote::id(),
                program_data: Pubkey::find_program_address(
                    &[ecol_lote::id().as_ref()],
                    &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
                )
                .0,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn update_global(&mut self, admin: &Keypair, params: ConfigParams) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminUpdateGlobal { params },
            ctas::AdminUpdateGlobal {
                admin: admin.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[admin])
    }

    pub fn set_modo(&mut self, modo: ModoOperacao) -> Result<TransactionMetadata, TransactionError> {
        let admin = self.admin.insecure_clone();
        let ix = self.ix(
            ixs::AdminSetModo { modo },
            ctas::AdminUpdateGlobal {
                admin: admin.pubkey(),
                config: config_pda(),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&admin])
    }

    pub fn registrar(&mut self, carteira: Pubkey, papel: Papel) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRegisterParticipante { carteira, papel, kyc_hash: h("kyc") },
            ctas::OperadorRegisterParticipante {
                payer: operador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_ativo(&mut self, carteira: Pubkey, ativo: bool) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSetParticipanteAtivo { ativo },
            ctas::OperadorSetParticipanteAtivo {
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn criar_lote_por(&mut self, cooperativa: &Keypair, lote_id: u64, peso_g: u64) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CooperativaCreateLote {
                lote_id,
                material: Material::Plastico,
                peso_g,
                coletor_ref: h("coletor+salt"),
                evidencias_hash: h("fotos+ticket"),
            },
            ctas::CooperativaCreateLote {
                payer: self.operador.pubkey(),
                cooperativa: cooperativa.pubkey(),
                cooperativa_part: participante_pda(&cooperativa.pubkey()),
                config: config_pda(),
                lote: lote_pda(&cooperativa.pubkey(), lote_id),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[cooperativa])
    }

    pub fn criar_lote(&mut self, lote_id: u64) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.criar_lote_por(&coop, lote_id, PESO_G)
    }

    pub fn anunciar_por(&mut self, cooperativa: &Keypair, lote: Pubkey, prazo_leilao: i64, preco_minimo_centavos: u64) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CooperativaListLote { prazo_leilao, preco_minimo_centavos },
            ctas::CooperativaListLote {
                cooperativa: cooperativa.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[cooperativa])
    }

    pub fn anunciar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.anunciar_por(&coop, lote, T0 + 3600, 25_000)
    }

    pub fn encerrar_sem_lance(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorCloseLeilaoSemLance {},
            ctas::OperadorCloseLeilaoSemLance {
                operador: operador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn vender_com(&mut self, industria: &Keypair, intermediador: &Keypair, lote: Pubkey, valor_centavos: u64) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IndustriaAcceptVenda { valor_centavos, escrow_ref_hash: h("escrow-123"), lances_root: h("lances") },
            ctas::IndustriaAcceptVenda {
                industria: industria.pubkey(),
                industria_part: participante_pda(&industria.pubkey()),
                intermediador: intermediador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[industria, intermediador])
    }

    pub fn vender(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (ind, inter) = (self.industria.insecure_clone(), self.intermediador.insecure_clone());
        self.vender_com(&ind, &inter, lote, 25_000)
    }

    pub fn retirar_com(&mut self, transportador: &Keypair, cooperativa: &Keypair, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::TransportadorPickupLote {},
            ctas::TransportadorPickupLote {
                transportador: transportador.pubkey(),
                transportador_part: participante_pda(&transportador.pubkey()),
                cooperativa: cooperativa.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[transportador, cooperativa])
    }

    pub fn retirar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (t, c) = (self.transportador.insecure_clone(), self.cooperativa.insecure_clone());
        self.retirar_com(&t, &c, lote)
    }

    pub fn receber_por(&mut self, industria: &Keypair, lote: Pubkey, peso_recebido_g: u64) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IndustriaConfirmRecebimento { peso_recebido_g },
            ctas::IndustriaConfirmRecebimento {
                industria: industria.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[industria])
    }

    pub fn receber(&mut self, lote: Pubkey, peso_recebido_g: u64) -> Result<TransactionMetadata, TransactionError> {
        let ind = self.industria.insecure_clone();
        self.receber_por(&ind, lote, peso_recebido_g)
    }

    pub fn liberar_por(&mut self, intermediador: &Keypair, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IntermediadorConfirmLiberacao { liberacao_ref_hash: h("pix-liberacao") },
            ctas::IntermediadorConfirmLiberacao {
                intermediador: intermediador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[intermediador])
    }

    pub fn liberar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let inter = self.intermediador.insecure_clone();
        self.liberar_por(&inter, lote)
    }

    pub fn abrir_disputa(&mut self, parte: &Keypair, lote: Pubkey, motivo: MotivoDisputa) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ParteOpenDisputa { motivo },
            ctas::ParteOpenDisputa {
                parte: parte.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[parte])
    }

    pub fn crank_prazo(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let cranker = Keypair::new();
        let ix = self.ix(
            ixs::PublicCrankPrazoExpirado {},
            ctas::PublicCrankPrazoExpirado {
                cranker: cranker.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&cranker])
    }

    pub fn resolver(&mut self, arbitro: &Keypair, intermediador: Option<&Keypair>, lote: Pubkey, decisao: DecisaoDisputa) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::ArbitroResolveDisputa { decisao },
            ctas::ArbitroResolveDisputa {
                arbitro: arbitro.pubkey(),
                intermediador: intermediador.map(|k| k.pubkey()),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        let mut assinantes = vec![arbitro];
        if let Some(i) = intermediador {
            assinantes.push(i);
        }
        self.enviar(ix, &assinantes)
    }

    pub fn agregar_por(&mut self, autoridade: &Keypair, lote: Pubkey, credito: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CreditoMarkAgregado { credito },
            ctas::CreditoMarkAgregado {
                credito_autoridade: autoridade.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[autoridade])
    }

    pub fn lote(&self, lote: Pubkey) -> Lote {
        let conta = self.svm.get_account(&lote).unwrap();
        Lote::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn config(&self) -> GlobalConfig {
        let conta = self.svm.get_account(&config_pda()).unwrap();
        GlobalConfig::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn participante(&self, carteira: &Pubkey) -> Participante {
        let conta = self.svm.get_account(&participante_pda(carteira)).unwrap();
        Participante::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    /// Cria o lote `lote_id` e o leva até `Recebido`.
    pub fn lote_ate_recebido(&mut self, lote_id: u64) -> Pubkey {
        let lote = lote_pda(&self.cooperativa.pubkey(), lote_id);
        self.criar_lote(lote_id).unwrap();
        self.anunciar(lote).unwrap();
        self.vender(lote).unwrap();
        self.retirar(lote).unwrap();
        self.receber(lote, PESO_G).unwrap();
        assert_eq!(self.lote(lote).estado, EstadoLote::Recebido);
        lote
    }
}

/// Afirma que a transação falhou com o erro `esperado` do programa.
#[track_caller]
pub fn assert_erro(res: Result<TransactionMetadata, TransactionError>, esperado: ecol_lote::error::EcolLoteError) {
    let esperado_nome = format!("{esperado:?}");
    let codigo_esperado = codigo(esperado);
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(c, codigo_esperado, "esperava {esperado_nome} ({codigo_esperado}), veio {c}")
        }
        outro => panic!("esperava {esperado_nome}, veio {outro:?}"),
    }
}
