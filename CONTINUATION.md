# CONTINUATION — EcolChain

Documento vivo para retomar o trabalho entre sessões. Atualizar ao fim de cada sessão
(seção **Histórico** + ajustar **Pendências** e **Próximos passos**).

- **Diagrama de referência:** `docs/Proposta_04.excalidraw` (v04, com as decisões de 2026-09-28; `Proposta_03` mantido como original)
- **Última atualização:** 2026-09-28

---

## 1. Contexto do projeto

EcolChain = reciclagem rastreável + blockchain (Solana) + créditos de carbono.

**Fluxo físico:** Coletor → Cooperativa (pesa/classifica) → Transportador → Indústria.
**Marketplace de lotes:** cooperativa anuncia lote (NFT), indústrias dão lances, pagamento em escrow liberado na entrega.
**Trilha on-chain:** ① mint do NFT → ② venda + custódia → ③ recebimento (libera escrow) → ④ NFT "reciclado" → ⑤ validação (Verra / Gold Standard) e emissão de tCO2e.
**Repartição (smart contract) da receita dos créditos:**

| Participante | Padrão | Faixa | Forma |
|---|---|---|---|
| Coletor | 5% | fixo | cupons Zupy |
| Cooperativa | 10% | fixo | cupons Zupy |
| Transportador | 5% | fixo | cupons Zupy |
| Indústria | 65% | até 79% | cupons Zupy |
| EcolChain | 15% | até 1% | R$ / stablecoin (antes: tokens) |

Indústria + EcolChain negociam entre si a fatia de 80%. Exemplo do diagrama: 1 t PET, lote a US$ 50, 1 crédito vendido a US$ 50.

---

## 2. Foco atual

**Camada blockchain Solana** (decidido pelo usuário em 2026-09-28).

---

## 2b. Implementação on-chain (em andamento)

- **Plano:** `/root/.claude/plans/elabore-um-plano-para-elegant-journal.md` (3 programs: `ecol_lote`, `ecol_credito`, `ecol_reparticao`; fases F0–F5)
- **Workspace Anchor:** `onchain/` (Anchor 1.1.2, Solana CLI 3.1.10, Surfpool 1.6.0, testes LiteSVM 0.10)
- **ADRs:** `docs/adr/` (as 8 decisões de 2026-09-28)
- **Rodar:** `cd onchain && NO_DNA=1 anchor test`

| Fase | Status |
|---|---|
| F0 Setup (workspace, ADRs) | ✅ (CI pendente: repositório sem remoto) |
| F1 `ecol_lote` núcleo | ✅ |
| F2 Balança Ed25519, Core asset, atestações | ✅ + auditoria (solana-ai-kit) com 10 achados corrigidos — `docs/security-audit-2026-09-28.md` |
| F3 `ecol_credito` | ⏳ próximo (tratar achado 14 da auditoria) |
| F4 `ecol_reparticao` | — |
| F5 Codama, devnet, Squads, segurança | — |

**Estado atual do `ecol_lote`:** 21 instruções; 46 testes (5 unitários + 41 LiteSVM: fluxo, autorização, pesagem/asset, regressões da auditoria, compute units); clippy estrito limpo. Custo medido ≈ 0,0084 SOL/lote (ver ADR 0003).

**Boas práticas:** seguir o [solana-ai-kit](https://github.com/solanabr/solana-ai-kit) (pedido do usuário em 2026-09-28): checklists `/audit-solana`, `safe-solana-builder`, Trail of Bits; aritmética checada, sem `unwrap` em produção, `init_if_needed` proibido, auditoria antes de fechar cada fase.

**Ajustes em relação ao plano (F2):**
- Seeds da balança = `["balanca", dispositivo]` (um equipamento, um dono).
- Titularidade do Core asset acompanha a custódia: cooperativa → transportador (retirada) → indústria (recebimento).
- Sem plugin `Attributes` no asset: o estado canônico fica na PDA; o asset é derivável do lote (`["asset", lote]`).
- Surfpool: a lógica de tempo foi coberta no LiteSVM (`set_sysvar` de `Clock`); o teste de integração em Surfpool fica para o E2E da F5.
- Correções da auditoria: disputa julgada não reabre, decisão `Liberar` (parcial), venda co-assinada pelo operador, crank só em Normal + carência 48 h, participante inativo sem novos compromissos, chaves de config distintas, admin em dois passos, liberação em `SomenteDisputas`, anúncio com teto/cancelamento, tolerância fixada na venda, fee payer separado do operador.
- Abertos: `close` de contas terminais (achado 13), confiança na PDA do `ecol_credito` (14, na F3), taxa de ~0,0015 SOL do Core a confirmar.

**Ajustes em relação ao plano (F1):**
- `cooperativa_list_lote` cobre anúncio e reanúncio (uma instrução só).
- Sem `seq_global` no `GlobalConfig`: seria uma conta com write lock em toda transação. O `seq` é por lote.
- Disputa resolvida com `Prosseguir { novo_peso_g }` (volta à etapa de origem, renovando o prazo se for antes da entrega) ou `Reembolsar` (co-assinado pelo intermediador).
- Modos de operação: `Normal`, `SomenteDisputas` (disputas, crank e árbitro continuam), `Pausado`.
- `admin_init_global` exige a upgrade authority do programa.
- Na F1, `cooperativa_create_lote` recebe o peso direto; a F2 passa a exigir a assinatura Ed25519 da balança.

---

## 3. Achados da análise da camada Solana

### Modelo de dados
- NFT hoje acumula "registro do lote" e "motor do processo". Proposta: **PDA de estado por lote** (máquina de estados explícita) + NFT apenas como recibo.
- Estados sugeridos:
  ```
  Criado → Anunciado → Vendido(escrow) → EmTransporte → Recebido → Reciclado → Agregado(crédito)
                 ↘ SemLance → Reanunciado        ↘ EmDisputa → (Reclassificado | Reembolsado)
  ```
- Diagrama junta ③ e ④ e não mostra Anunciado, EmDisputa/Pausado, Reembolsado.
- cNFT exige prova Merkle via indexador DAS a cada transição → dependência de RPC/indexador.
- NFT **não pode ser livremente transferível** (burlaria marketplace/escrow): freeze ou delegate do programa.

### Oráculo / confiança
- On-chain prova "chave X declarou Y em T", não a realidade física.
- Balança deve ter keypair próprio e assinar a leitura (verificação Ed25519 on-chain).
- Se a EcolChain assina por todos (custodial), a trilha perde valor probatório → carteiras embarcadas/MPC por participante; EcolChain como fee payer (ou Kora, gasless).
- **LGPD:** metadados "carteiras participantes" + "data/local" podem identificar coletores → on-chain só hashes com salt; fotos off-chain (Arweave/IPFS/S3).

### Escrow
- Escrow = dinheiro retido por terceiro neutro até a entrega ser confirmada.
- Opção A: R$ off-chain com intermediador (Pix/conta garantia), blockchain só registra.
- Opção B: stablecoin (USDC/BRZ) em vault PDA por lote, liberação automática.
- **Recomendação: começar pela opção A.**
- Precisa de prazo/timeout, crank de reembolso e árbitro (multisig) para disputas.

### Repartição
- A auditoria quebra na Zupy (divisão por participante é off-chain) → gravar direitos por participante on-chain ou Zupy publicar recibos on-chain.
- Crédito agrega muitos lotes; limite de 64 contas por transação → modelo **pull / Merkle distributor**.
- Percentuais em PDA `Config` (basis points) com limites validados on-chain; regra de resíduo de arredondamento; alteração só via multisig (Squads) + timelock.
- Token da EcolChain indefinido (mint authority, conversão R$→token, risco CVM). Stablecoin simplifica.

### Crédito de carbono on-chain
- Crédito nasce no registro (Verra/GS) off-chain → precisa de ponte: token fungível por vintage, número de série do registro on-chain, lock/retire no registro, burn na aposentadoria.
- Ausente no diagrama; é onde ocorreria dupla contagem.

### Auditoria
- Com cNFT, auditor precisa de RPC de arquivo + indexador; publicar verificador open-source.
- Upgrade authority em multisig + builds verificáveis (`solana-verify`).
- Eventos estruturados com número de sequência por lote.

### Quadro de custos (validação)
- 5.000 lamports/assinatura: correto, mas é **por assinatura** (multisig multiplica).
- Ciclo ~6 tx ≈ US$ 0,006–0,01: ok.
- NFT padrão ~0,012 SOL é majoritariamente **rent = depósito recuperável**, não despesa.
- cNFT: faltou custo inicial da árvore Merkle.
- Não citados: rent da PDA de estado e das contas de escrow (recuperáveis), indexador DAS obrigatório, armazenamento de evidências, atestação de balanças.
- Meio-termo não citado: **Metaplex Core asset** (~0,003 SOL, conta única, plugins). Validar valores vigentes.

### Arquitetura sugerida

| Componente | Proposta |
|---|---|
| Programa EcolChain (Anchor) | PDA `Lote` (state machine), PDA `Config` (bps + autoridades), vault de escrow por lote |
| Identidade do lote | Core asset não transferível (programa como delegate) — decidido |
| Pesagem | Atestação Ed25519 da balança verificada on-chain |
| Crédito | Registro on-chain do nº de série + lotes agregados + prova de aposentadoria (sem token) — decidido |
| Repartição | Merkle distributor (pull) com direito por participante |
| Governança | Squads multisig (upgrade, config, árbitro) |
| Taxas | EcolChain como fee payer / Kora; participantes só assinam |

---

## 4. Decisões tomadas

| Data | Decisão |
|---|---|
| 2026-09-28 | Foco da análise na camada blockchain Solana |
| 2026-09-28 | Escrow incluído como premissa em aberto no diagrama (sugestão inicial: R$ off-chain) |
| 2026-09-28 | **Escrow em R$ off-chain** via intermediador (Pix / conta garantia); programa só registra bloqueio/liberação |
| 2026-09-28 | **Sem token próprio**: parte da EcolChain paga em R$ / stablecoin (evita CVM e gestão de mint authority) |
| 2026-09-28 | **Identidade do lote = Metaplex Core asset** não transferível (freeze/permanent delegate do programa) + PDA `Lote` como estado |
| 2026-09-28 | **Chaves: carteira embutida/MPC por participante**; EcolChain como fee payer (ou Kora) |
| 2026-09-28 | **Crédito de carbono: só registro/referência on-chain** (nº de série, lotes agregados, prova de aposentadoria); sem tokenização |
| 2026-09-28 | **Coletor: R$ (Pix) na entrega** pela cooperativa + 5% do crédito em cupons Zupy depois |
| 2026-09-28 | **Zupy: direito por participante on-chain** (Merkle distributor); Zupy só resgata em cupons e publica recibo on-chain |
| 2026-09-28 | **Governança: Squads multisig com terceiros** (ex. 3-de-5: EcolChain, cooperativas, indústria, auditor) + timelock; controla upgrade, `Config` e árbitro |

---

## 5. Pendências / premissas em aberto

- [x] Ativo do escrow → R$ off-chain (ver Decisões)
- [x] Governança → Squads multisig com terceiros + timelock (definir membros e quórum)
- [x] Zupy → direito on-chain, Zupy só resgata (falta: SLA, liquidez de cupons)
- [x] Remuneração do coletor → R$ na entrega + cupons
- [ ] Enquadramento regulatório: NFT/token, cupons, créditos de carbono, LGPD
- [x] Token da EcolChain → sem token próprio; R$ / stablecoin
- [x] Representação do NFT → Core asset
- [x] Custódia de chaves → embedded/MPC + fee payer
- [x] Crédito de carbono → só registro/referência on-chain (sem ponte/token)

---

## 6. Próximos passos (opções oferecidas)

- [x] Refletir as decisões de 2026-09-28 no diagrama → `Proposta_04.excalidraw`
- [ ] Redesenhar a faixa roxa (blockchain) do `.excalidraw` com a máquina de estados e componentes
- [x] Esqueleto Anchor da PDA `Lote` com as transições de estado → `onchain/programs/ecol_lote` (F1)
- [ ] Corrigir notação/legenda das setas e sobreposições do diagrama (ver achados gerais abaixo)

### Achados gerais do diagrama (fora do foco atual, para depois)
- Setas fora da legenda: `a_35` (preta, mas não é material), `a_37`/`a_97` (dinheiro tracejado), `a_52`–`a_54` (dados contínuos), `a_76` (sentido invertido).
- Falta seta de pagamento Indústria → Cooperativa e de entrega/aposentadoria do crédito ao comprador.
- `a_55` atravessa o texto `t_1009`; `t_83` vaza do retângulo; setas sem binding.
- Negócio: "1 t = 1 crédito = US$ 50" frágil (varia por material; crédito de plástico ≠ tCO2e), emissão por lote inviável (agregar), adicionalidade, dupla contagem com a Indústria, receita da EcolChain pouco clara, mistura R$/US$/tokens/cupons.

---

## 7. Histórico

### 2026-09-28
- Analisado `docs/Proposta_03.excalidraw` (visão geral: negócio, fluxos, notação, layout).
- Aprofundada a camada Solana (seção 3).
- Explicado o conceito de escrow e as opções A/B.
- Diagrama alterado: nova linha em "❓ PREMISSAS EM ABERTO" (`t_1014`), retângulo `r_1013` +16 px de altura.
- Criado este `CONTINUATION.md`.
- Sessão 4: F2 concluída (balança Ed25519, Core asset); auditoria com checklists do solana-ai-kit, 10 achados corrigidos com testes de regressão; custo real medido.
- Sessão 3: plano dos programs aprovado; F0 (workspace `onchain/`, ADRs, git init) e F1 (`ecol_lote` com máquina de estados, 18 testes) concluídas.
- Sessão 2: gerado `docs/Proposta_04.excalidraw` (novo quadro ✅ Decisões, premissas em aberto revisadas, tokens → R$, Core asset, coletor R$ na entrega, Zupy resgata direitos, multisig, custos revisados).
- Sessão 2: fechadas 8 premissas (escrow R$, sem token próprio, Core asset, carteira embutida/MPC, crédito só referência, coletor R$ + cupons, direito on-chain com Zupy resgatando, multisig com terceiros).
