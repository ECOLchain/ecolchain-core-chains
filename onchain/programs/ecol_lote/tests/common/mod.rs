//! Ambiente LiteSVM compartilhado pelos testes do `ecol_lote`.
#![allow(dead_code)]

use {
    anchor_lang::{
        prelude::{Clock, Pubkey},
        solana_program::{
            instruction::{AccountMeta, Instruction},
            system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    ecol_lote::{
        accounts as ctas, instruction as ixs,
        pesagem::{mensagem, TipoPesagem},
        ConfigParams, DadosEntrega, DadosLote, DecisaoDisputa, EstadoLote, GlobalConfig, Lote,
        Material, ModoOperacao, MotivoDisputa, Papel, Participante, SEED_ASSET,
        SEED_AUTORIDADE_ASSET, SEED_BALANCA, SEED_ENTREGA, SEED_GLOBAL, SEED_LOTE, SEED_MATERIAL,
        SEED_PARTICIPANTE,
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
/// Material cadastrado no `novo()` e usado nos lotes de teste.
pub const PLASTICO: u16 = 1;

pub struct Ambiente {
    pub svm: LiteSVM,
    /// Fee payer da EcolChain (Kora/relayer). Sem papel on-chain: pagar a taxa não autoriza nada.
    pub patrocinador: Keypair,
    pub upgrade_authority: Keypair,
    pub admin: Keypair,
    pub arbitro: Keypair,
    pub operador: Keypair,
    pub intermediador: Keypair,
    pub credito_autoridade: Keypair,
    pub cooperativa: Keypair,
    pub industria: Keypair,
    pub transportador: Keypair,
    /// Keypair do equipamento de pesagem instalado na cooperativa.
    pub balanca_coop: Keypair,
    /// Keypair do equipamento de pesagem instalado na indústria.
    pub balanca_ind: Keypair,
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

pub fn entrega_pda(cooperativa: &Pubkey, entrega_id: u64) -> Pubkey {
    pda(&[
        SEED_ENTREGA,
        cooperativa.as_ref(),
        &entrega_id.to_le_bytes(),
    ])
}

pub fn material_pda(codigo: u16) -> Pubkey {
    pda(&[SEED_MATERIAL, &codigo.to_le_bytes()])
}

pub fn balanca_pda(dispositivo: &Pubkey) -> Pubkey {
    pda(&[SEED_BALANCA, dispositivo.as_ref()])
}

pub fn asset_pda(lote: &Pubkey) -> Pubkey {
    pda(&[SEED_ASSET, lote.as_ref()])
}

pub fn autoridade_asset() -> Pubkey {
    pda(&[SEED_AUTORIDADE_ASSET])
}

/// Dono gravado no Core asset (layout BaseAssetV1: key u8 | owner Pubkey | ...).
pub fn dono_do_asset(svm: &LiteSVM, asset: &Pubkey) -> Pubkey {
    let conta = svm.get_account(asset).expect("asset não existe");
    assert_eq!(
        conta.owner,
        mpl_core::ID,
        "asset não pertence ao Metaplex Core"
    );
    Pubkey::try_from(&conta.data[1..33]).unwrap()
}

/// Instrução Ed25519 com a balança assinando a pesagem, como o app/firmware fará.
pub fn ix_pesagem(
    balanca: &Keypair,
    tipo: TipoPesagem,
    lote: &Pubkey,
    peso_g: u64,
    ts: i64,
) -> Instruction {
    let msg = mensagem(tipo, lote, peso_g, ts);
    let assinatura = balanca.sign_message(&msg);
    let assinatura: [u8; 64] = assinatura.as_ref().try_into().unwrap();
    solana_ed25519_program::new_ed25519_instruction_with_signature(
        &msg,
        &assinatura,
        &balanca.pubkey().to_bytes(),
    )
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
        let bytes = include_bytes!(concat!(
            env!("CARGO_TARGET_TMPDIR"),
            "/../deploy/ecol_lote.so"
        ));
        svm.add_program(ecol_lote::id(), bytes).unwrap();
        let core = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/mpl_core.so"
        ));
        svm.add_program(mpl_core::ID, core).unwrap();

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
            patrocinador: Keypair::new(),
            upgrade_authority,
            admin: Keypair::new(),
            arbitro: Keypair::new(),
            operador: Keypair::new(),
            intermediador: Keypair::new(),
            credito_autoridade: Keypair::new(),
            cooperativa: Keypair::new(),
            industria: Keypair::new(),
            transportador: Keypair::new(),
            balanca_coop: Keypair::new(),
            balanca_ind: Keypair::new(),
        };
        // Só o patrocinador (fee payer) precisa de SOL.
        let patrocinador = amb.patrocinador.pubkey();
        amb.svm.airdrop(&patrocinador, 10_000_000_000).unwrap();
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
        let (coop, bc) = (amb.cooperativa.pubkey(), amb.balanca_coop.pubkey());
        amb.registrar_balanca(coop, bc).unwrap();
        let (ind, bi) = (amb.industria.pubkey(), amb.balanca_ind.pubkey());
        amb.registrar_balanca(ind, bi).unwrap();
        amb.criar_material(PLASTICO, "Plástico").unwrap();
        amb
    }

    pub fn criar_material_por(
        &mut self,
        operador: &Keypair,
        codigo: u16,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorCreateMaterial {
                codigo,
                nome: nome.to_string(),
            },
            ctas::OperadorCreateMaterial {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    pub fn criar_material(
        &mut self,
        codigo: u16,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        self.criar_material_por(&operador, codigo, nome)
    }

    pub fn atualizar_material(
        &mut self,
        codigo: u16,
        nome: &str,
        ativo: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorUpdateMaterial {
                nome: nome.to_string(),
                ativo,
            },
            ctas::OperadorUpdateMaterial {
                operador: operador.pubkey(),
                config: config_pda(),
                material: material_pda(codigo),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn material(&self, codigo: u16) -> Material {
        let conta = self.svm.get_account(&material_pda(codigo)).unwrap();
        Material::try_deserialize(&mut conta.data.as_slice()).unwrap()
    }

    pub fn registrar_balanca(
        &mut self,
        dono: Pubkey,
        dispositivo: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        self.registrar_balanca_com_nome(dono, dispositivo, "Balança")
    }

    pub fn registrar_balanca_com_nome(
        &mut self,
        dono: Pubkey,
        dispositivo: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRegisterBalanca {
                dispositivo,
                nome: nome.into(),
            },
            ctas::OperadorRegisterBalanca {
                payer: self.patrocinador.pubkey(),
                operador: operador.pubkey(),
                config: config_pda(),
                dono_part: participante_pda(&dono),
                balanca: balanca_pda(&dispositivo),
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn set_balanca_ativa(
        &mut self,
        balanca: Pubkey,
        ativa: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorSetBalancaAtiva { ativa },
            ctas::OperadorSetBalancaAtiva {
                operador: operador.pubkey(),
                config: config_pda(),
                balanca,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[&operador])
    }

    pub fn agora(&self) -> i64 {
        self.svm.get_sysvar::<Clock>().unix_timestamp
    }

    pub fn dados(&self, lote_id: u64, peso_g: u64) -> DadosLote {
        DadosLote {
            lote_id,
            material: PLASTICO,
            peso_g,
            ts_pesagem: self.agora(),
            evidencias_hash: h("fotos+ticket"),
            uri: "https://arweave.net/lote-metadata.json".to_string(),
        }
    }

    /// Instrução de criação crua, sem a pesagem; os testes de ataque montam a transação à mão.
    pub fn ix_criar_lote(
        &self,
        cooperativa: &Pubkey,
        balanca: Pubkey,
        dados: DadosLote,
    ) -> Instruction {
        let lote = lote_pda(cooperativa, dados.lote_id);
        let material_cadastro = material_pda(dados.material);
        self.ix(
            ixs::CooperativaCreateLote { dados },
            ctas::CooperativaCreateLote {
                payer: self.patrocinador.pubkey(),
                cooperativa: *cooperativa,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                balanca,
                material_cadastro,
                lote,
                asset: asset_pda(&lote),
                autoridade_asset: autoridade_asset(),
                mpl_core_program: mpl_core::ID,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    pub fn custodia(&self, lote: &Pubkey) -> ctas::ContasCustodia {
        ctas::ContasCustodia {
            payer: self.patrocinador.pubkey(),
            asset: asset_pda(lote),
            autoridade_asset: autoridade_asset(),
            mpl_core_program: mpl_core::ID,
            system_program: system_program::ID,
        }
    }

    pub fn set_tempo(&mut self, unix_timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    /// Envia com o patrocinador como fee payer, como a EcolChain fará em produção.
    pub fn enviar(
        &mut self,
        ix: Instruction,
        assinantes: &[&Keypair],
    ) -> Result<TransactionMetadata, TransactionError> {
        self.enviar_varias(&[ix], assinantes)
    }

    pub fn enviar_varias(
        &mut self,
        ixs: &[Instruction],
        assinantes: &[&Keypair],
    ) -> Result<TransactionMetadata, TransactionError> {
        self.svm.expire_blockhash();
        let payer = self.patrocinador.insecure_clone();
        let blockhash = self.svm.latest_blockhash();
        let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
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
            arbitro: self.arbitro.pubkey(),
            operador: self.operador.pubkey(),
            intermediador: self.intermediador.pubkey(),
            credito_autoridade: self.credito_autoridade.pubkey(),
            prazo_entrega_s: PRAZO_ENTREGA_S,
            tolerancia_peso_bps: TOLERANCIA_BPS,
        }
    }

    pub fn init_global(
        &mut self,
        autoridade: &Keypair,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::AdminInitGlobal {
                admin: self.admin.pubkey(),
                params: self.params(),
            },
            ctas::AdminInitGlobal {
                payer: self.patrocinador.pubkey(),
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

    pub fn update_global(
        &mut self,
        admin: &Keypair,
        params: ConfigParams,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn set_modo(
        &mut self,
        modo: ModoOperacao,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn registrar(
        &mut self,
        carteira: Pubkey,
        papel: Papel,
    ) -> Result<TransactionMetadata, TransactionError> {
        self.registrar_com_nome(carteira, papel, "Participante")
    }

    pub fn registrar_com_nome(
        &mut self,
        carteira: Pubkey,
        papel: Papel,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        let ix = self.ix(
            ixs::OperadorRegisterParticipante {
                carteira,
                papel,
                kyc_hash: h("kyc"),
                nome: nome.into(),
            },
            ctas::OperadorRegisterParticipante {
                payer: self.patrocinador.pubkey(),
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

    pub fn set_ativo(
        &mut self,
        carteira: Pubkey,
        ativo: bool,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn set_balanca_nome_por(
        &mut self,
        operador: &Keypair,
        dispositivo: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorSetBalancaNome { nome: nome.into() },
            ctas::OperadorSetBalancaNome {
                operador: operador.pubkey(),
                config: config_pda(),
                balanca: balanca_pda(&dispositivo),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    pub fn set_nome_por(
        &mut self,
        operador: &Keypair,
        carteira: Pubkey,
        nome: &str,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::OperadorSetParticipanteNome { nome: nome.into() },
            ctas::OperadorSetParticipanteNome {
                operador: operador.pubkey(),
                config: config_pda(),
                participante: participante_pda(&carteira),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[operador])
    }

    /// Criação normal: pesagem assinada pela `balanca` + instrução do programa.
    pub fn criar_lote_com(
        &mut self,
        cooperativa: &Keypair,
        balanca: &Keypair,
        lote_id: u64,
        peso_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let dados = self.dados(lote_id, peso_g);
        let lote = lote_pda(&cooperativa.pubkey(), lote_id);
        let pesagem = ix_pesagem(
            balanca,
            TipoPesagem::Origem,
            &lote,
            peso_g,
            dados.ts_pesagem,
        );
        let criar =
            self.ix_criar_lote(&cooperativa.pubkey(), balanca_pda(&balanca.pubkey()), dados);
        let res = self.enviar_varias(&[pesagem, criar], &[cooperativa]);
        if res.is_ok() {
            // Lote montado: uma entrega de coletor com o mesmo peso (id da entrega = id do lote).
            self.registrar_entrega(cooperativa, balanca, lote_id, peso_g, PLASTICO)
                .unwrap();
            let entrega = entrega_pda(&cooperativa.pubkey(), lote_id);
            self.add_entregas(cooperativa, lote, &[entrega]).unwrap();
        }
        res
    }

    pub fn dados_entrega(&self, entrega_id: u64, peso_g: u64, material: u16) -> DadosEntrega {
        DadosEntrega {
            entrega_id,
            coletor_ref: h(&format!("coletor-{entrega_id}+salt")),
            material,
            peso_g,
            ts_pesagem: self.agora(),
        }
    }

    pub fn ix_registrar_entrega(
        &self,
        cooperativa: &Pubkey,
        balanca: Pubkey,
        dados: DadosEntrega,
    ) -> Instruction {
        let entrega = entrega_pda(cooperativa, dados.entrega_id);
        let material_cadastro = material_pda(dados.material);
        self.ix(
            ixs::CooperativaRegisterEntrega { dados },
            ctas::CooperativaRegisterEntrega {
                payer: self.patrocinador.pubkey(),
                cooperativa: *cooperativa,
                cooperativa_part: participante_pda(cooperativa),
                config: config_pda(),
                balanca,
                material_cadastro,
                entrega,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                system_program: system_program::ID,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    /// Entrega pesada e assinada pela `balanca` da cooperativa.
    pub fn registrar_entrega(
        &mut self,
        cooperativa: &Keypair,
        balanca: &Keypair,
        entrega_id: u64,
        peso_g: u64,
        material: u16,
    ) -> Result<TransactionMetadata, TransactionError> {
        let dados = self.dados_entrega(entrega_id, peso_g, material);
        let entrega = entrega_pda(&cooperativa.pubkey(), entrega_id);
        let pesagem = ix_pesagem(
            balanca,
            TipoPesagem::Entrega,
            &entrega,
            peso_g,
            dados.ts_pesagem,
        );
        let ix =
            self.ix_registrar_entrega(&cooperativa.pubkey(), balanca_pda(&balanca.pubkey()), dados);
        self.enviar_varias(&[pesagem, ix], &[cooperativa])
    }

    pub fn add_entregas(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        entregas: &[Pubkey],
    ) -> Result<TransactionMetadata, TransactionError> {
        let mut ix = self.ix(
            ixs::CooperativaAddEntregas {},
            ctas::CooperativaAddEntregas {
                cooperativa: cooperativa.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        ix.accounts
            .extend(entregas.iter().map(|e| AccountMeta::new(*e, false)));
        self.enviar(ix, &[cooperativa])
    }

    /// Criação com a balança da cooperativa principal.
    pub fn criar_lote_por(
        &mut self,
        cooperativa: &Keypair,
        lote_id: u64,
        peso_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let balanca = self.balanca_coop.insecure_clone();
        self.criar_lote_com(cooperativa, &balanca, lote_id, peso_g)
    }

    pub fn criar_lote(&mut self, lote_id: u64) -> Result<TransactionMetadata, TransactionError> {
        let coop = self.cooperativa.insecure_clone();
        self.criar_lote_por(&coop, lote_id, PESO_G)
    }

    pub fn anunciar_por(
        &mut self,
        cooperativa: &Keypair,
        lote: Pubkey,
        prazo_leilao: i64,
        preco_minimo_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::CooperativaListLote {
                prazo_leilao,
                preco_minimo_centavos,
            },
            ctas::CooperativaListLote {
                cooperativa: cooperativa.pubkey(),
                cooperativa_part: participante_pda(&cooperativa.pubkey()),
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

    pub fn encerrar_sem_lance(
        &mut self,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn vender_com(
        &mut self,
        industria: &Keypair,
        intermediador: &Keypair,
        lote: Pubkey,
        valor_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let operador = self.operador.insecure_clone();
        self.vender_completo(industria, &operador, intermediador, lote, valor_centavos)
    }

    pub fn vender_completo(
        &mut self,
        industria: &Keypair,
        operador: &Keypair,
        intermediador: &Keypair,
        lote: Pubkey,
        valor_centavos: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let cooperativa = self.lote(lote).cooperativa;
        let ix = self.ix(
            ixs::IndustriaAcceptVenda {
                valor_centavos,
                escrow_ref_hash: h("escrow-123"),
                lances_root: h("lances"),
            },
            ctas::IndustriaAcceptVenda {
                industria: industria.pubkey(),
                industria_part: participante_pda(&industria.pubkey()),
                cooperativa_part: participante_pda(&cooperativa),
                operador: operador.pubkey(),
                intermediador: intermediador.pubkey(),
                config: config_pda(),
                lote,
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[industria, operador, intermediador])
    }

    pub fn vender(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (ind, inter) = (
            self.industria.insecure_clone(),
            self.intermediador.insecure_clone(),
        );
        self.vender_com(&ind, &inter, lote, 25_000)
    }

    pub fn retirar_com(
        &mut self,
        transportador: &Keypair,
        cooperativa: &Keypair,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::TransportadorPickupLote {},
            ctas::TransportadorPickupLote {
                transportador: transportador.pubkey(),
                transportador_part: participante_pda(&transportador.pubkey()),
                cooperativa: cooperativa.pubkey(),
                cooperativa_part: participante_pda(&cooperativa.pubkey()),
                config: config_pda(),
                lote,
                custodia: self.custodia(&lote),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        );
        self.enviar(ix, &[transportador, cooperativa])
    }

    pub fn retirar(&mut self, lote: Pubkey) -> Result<TransactionMetadata, TransactionError> {
        let (t, c) = (
            self.transportador.insecure_clone(),
            self.cooperativa.insecure_clone(),
        );
        self.retirar_com(&t, &c, lote)
    }

    pub fn ix_receber(
        &self,
        industria: &Pubkey,
        lote: Pubkey,
        peso_recebido_g: u64,
        balanca: Option<Pubkey>,
        ts_pesagem: Option<i64>,
    ) -> Instruction {
        self.ix(
            ixs::IndustriaConfirmRecebimento {
                peso_recebido_g,
                ts_pesagem,
            },
            ctas::IndustriaConfirmRecebimento {
                industria: *industria,
                balanca,
                instrucoes_sysvar: solana_instructions_sysvar::ID,
                config: config_pda(),
                lote,
                custodia: self.custodia(&lote),
                event_authority: event_authority(),
                program: ecol_lote::id(),
            },
        )
    }

    /// Recebimento sem pesagem atestada.
    pub fn receber_por(
        &mut self,
        industria: &Keypair,
        lote: Pubkey,
        peso_recebido_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix_receber(&industria.pubkey(), lote, peso_recebido_g, None, None);
        self.enviar(ix, &[industria])
    }

    pub fn receber(
        &mut self,
        lote: Pubkey,
        peso_recebido_g: u64,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ind = self.industria.insecure_clone();
        self.receber_por(&ind, lote, peso_recebido_g)
    }

    pub fn liberar_por(
        &mut self,
        intermediador: &Keypair,
        lote: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
        let ix = self.ix(
            ixs::IntermediadorConfirmLiberacao {
                liberacao_ref_hash: h("pix-liberacao"),
            },
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

    pub fn abrir_disputa(
        &mut self,
        parte: &Keypair,
        lote: Pubkey,
        motivo: MotivoDisputa,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn resolver(
        &mut self,
        arbitro: &Keypair,
        intermediador: Option<&Keypair>,
        lote: Pubkey,
        decisao: DecisaoDisputa,
    ) -> Result<TransactionMetadata, TransactionError> {
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

    pub fn agregar_por(
        &mut self,
        autoridade: &Keypair,
        lote: Pubkey,
        credito: Pubkey,
    ) -> Result<TransactionMetadata, TransactionError> {
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
pub fn assert_erro(
    res: Result<TransactionMetadata, TransactionError>,
    esperado: ecol_lote::error::EcolLoteError,
) {
    let esperado_nome = format!("{esperado:?}");
    let codigo_esperado = codigo(esperado);
    match res {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(c))) => {
            assert_eq!(
                c, codigo_esperado,
                "esperava {esperado_nome} ({codigo_esperado}), veio {c}"
            )
        }
        outro => panic!("esperava {esperado_nome}, veio {outro:?}"),
    }
}
