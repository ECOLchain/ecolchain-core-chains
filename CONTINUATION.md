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
| F0 Setup (workspace, ADRs) | ✅ (repositório no GitHub; CI pendente) |
| F1 `ecol_lote` núcleo | ✅ |
| F2 Balança Ed25519, Core asset, atestações | ✅ + auditoria (solana-ai-kit) com 10 achados corrigidos — `docs/security-audit-2026-09-28.md` |
| F3 `ecol_credito` | ✅ 13 instruções, 19 testes (CPI real com o `ecol_lote`); achado 14, C1 e C2 resolvidos |
| F4 `ecol_reparticao` | ✅ 8 instruções, 17 testes (3 programas encadeados); riscos R1–R3 registrados |
| F5 Codama, devnet, Squads, segurança | ✅ clientes Codama, gerador TS da árvore, E2E Surfpool, testes de propriedade; **deploy devnet verificável + Squads 3 de 5** (2026-09-28) |

**Estado atual do `ecol_lote`:** 30 instruções (materiais, guardião e entregas desde a sessão 10; nomes de participante e balança na sessão 12); 64 testes (5 unitários + 41 LiteSVM: fluxo, autorização, pesagem/asset, regressões da auditoria, compute units); clippy estrito limpo. Custo medido ≈ 0,0084 SOL/lote (ver ADR 0003).

**Boas práticas:** seguir o [solana-ai-kit](https://github.com/solanabr/solana-ai-kit) (pedido do usuário em 2026-09-28): checklists `/audit-solana`, `safe-solana-builder`, Trail of Bits; aritmética checada, sem `unwrap` em produção, `init_if_needed` proibido, auditoria antes de fechar cada fase.

**`ecol_credito` (F3):** `Credito` = PDA `["credito", credito_id]` (o serial só existe após a emissão; unicidade via PDA `["serial", padrão, sha256(serial)]`). Estados `EmFormacao → EmValidacao → Emitido → Vendido → Aposentado`, ou `Rejeitado`. Agregação por CPI assinada pela PDA `["autoridade_credito"]` (é a `credito_autoridade` do `ecol_lote`), até 10 lotes/tx, mesmo material; `lotes_hash` encadeado (`sha256(h ‖ lote ‖ peso)`) em vez de raiz Merkle off-chain. Papel novo: **registrador** (atesta emissão, rejeição e aposentadoria no registro). Venda assinada por operador + comprador + intermediador.

**`ecol_reparticao` (F4):** `Distribuicao` = PDA `["dist", credito]` (uma por crédito vendido). Tetos por papel calculados on-chain: `floor(receita × bps / 10.000)` para coletor (5%), cooperativa (10%), transportador (5%) e indústria (65–79%, negociado); EcolChain = restante (1–15% + resíduo). Árvore Merkle (folha `0x00`, nó `0x01`, pares ordenados) distribui dentro de cada papel; resgate por Zupy + beneficiário (cadastrado no `ecol_lote` com o papel da folha), bitmap contra resgate duplo, teto por papel. Estados `Pendente → Financiada` (intermediador atesta repasse à Zupy e à EcolChain). Exemplo do diagrama (US$ 50): 250 / 500 / 250 / 3.250 e EcolChain 750. Gerador da árvore em Rust (`ecol_reparticao::merkle`); script TS fica para a F5.

**Riscos residuais da F4 (decidir antes de mainnet):** R1 divisão dentro do papel depende do operador; R2 percentual da indústria informado só pelo operador; R3 direitos não resgatados não expiram.

**F5 (clientes e testes):** `onchain/clients` — clientes `@solana/kit` 8 gerados pelo Codama (`npm run gerar`), `src/pdas.ts`, `src/pesagem.ts` (Ed25519 da balança), `src/arvore.ts` (divisão por peso com maior resto, mesmo hash do Rust; vetor compartilhado). E2E no Surfpool embarcado (`npm run e2e`). Transações v0 porque o plugin-rpc 0.18 exigido pelo Surfpool 1.6 não monta v1. Testes: 90 Rust + 4 TS + 2 E2E.

**Deploy devnet — decidido (2026-09-28):** esperar o build verificável; Squads com 5 chaves de teste, quórum 3 (vault 0 = upgrade authority + `admin`; vault 1 = `arbitro`; timelock 1 h na devnet).

**Deploy devnet — feito (2026-09-28)** com `onchain/scripts/deploy-devnet.sh` (build verificável `quay.io/ottersec/anchor:v1.1.2`; hash on-chain = build nos 3):

| Item | Endereço |
|---|---|
| `ecol_lote` | `AGAi3h9KqSCzx7KTiejyEKHb1kVPPUQ2nKzjSUrGSvdD` (hash `1a673abc…72ed`) |
| `ecol_credito` | `FZtf8txpz79i4dcXcwP1NqqSMJUoZJKwjJSfjD3Y1doQ` (hash `31be82e8…5ce0`) |
| `ecol_reparticao` | `HTLtLauuXt9PqBzJfHaWPpeJUdtWpDcAa5ZsMSnP8k8K` (hash `722c20e8…af2d`) |
| Squads multisig (3 de 5, timelock 1 h) | `9W2HFHrGQ4sTtCaRCSeZ2ydiyuGAzg1Exb5jYFw3ZFjd` |
| Vault 0 (upgrade authority + `admin`) | `5QLoaHGatRQnhiJxFBXhvkJ9GdDbzhCvySXwNCKiwXHj` |
| Vault 1 (`arbitro`) | `BKgC9T4fCyMA1AMe3fNJTquQjvufgjQcdCkXjjV1yh7G` |
| operador / intermediador / registrador / zupy | `AURz…5ZGa` / `39J3…82Ez` / `p2e2…mYz` / `GTKW…9cb` |

Configs inicializadas nos 3 programas. Custo ≈ 7,5 SOL (14,05 → 6,57). Chaves de teste e `squads.json` em `onchain/.devnet/` (fora do git); backup das keypairs dos programas em `~/backup-ecolchain/`. Upgrades e mudanças de config agora só por proposta no Squads.

**Decisões C1 e C2 (usuário, 2026-09-28):**
- C1: desagregar com prazo — crédito em formação por mais de 180 dias vira `Abandonado`; em `Rejeitado`/`Abandonado`, o crank público `public_release_lotes` devolve os lotes a `Reciclado` (`ecol_lote::credito_mark_desagregado`).
- C2: o intermediador de pagamento co-assina a venda do crédito (`pagamento_ref_hash`); é papel no `CreditoConfig`.

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
| 2026-09-28 | **Entregas dos coletores on-chain**, uma por coletor e material, com a balança da cooperativa assinando; lotes montados a partir das entregas; **lances fora da blockchain** |
| 2026-09-28 | **Início do processo (ADR 0010):** lote de origem (= `Entrega`) com ou sem coletor (origem: coletor, triagem própria, doação, compra, avulso + hash da referência); união em **lote de venda** (mesma cooperativa, mesmo material, dois níveis fixos, sempre via lote de venda), pesado de novo ao fechar; desfazer antes da venda; NFT só no lote de venda; consumo total pela indústria; 5% do coletor sobre peso sem coletor vai para a EcolChain |
| 2026-09-28 | ~~Governança: Squads multisig com terceiros~~ → **substituída (ADR 0009): Squads 3 de 5 só com chaves da EcolChain**; timelock 0 na devnet e 24 h na mainnet; controla upgrade, configs e árbitro |

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
- [x] **Implementar a ADR 0010** (lote de origem sem coletor, lote de venda com fechar/desmontar, teto proporcional dos coletores); tolerância −10%/+2%, entregas antigas = origem Coletor, busca pela referência da doação na trilha pública
- [ ] NF/recibo no lote de origem com coletor (upgrade do `ecol_lote`; aguardando decisão do usuário)
- [ ] RPC próprio para a devnet/mainnet (o público limita a trilha com 429)
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
- Sessão 12 (cont.): **Retiradas com QR (web `9013704`).** `transportador_pickup_lote` exige cooperativa + transportador, em aparelhos diferentes: a cooperativa identifica o transportador (QR da Minha carteira ou lista), assina e mostra um QR compacto `ecolchain:retirada:v1:<lote>:<coop>:<transp>:<blockhash>:<último bloco>:<assinatura>`; o aparelho do transportador remonta a mesma transação a partir da blockchain, verifica a assinatura Ed25519 da cooperativa, assina e envia (`web/src/solana/retirada.ts`). A primeira versão, com a transação inteira no QR, ficou densa demais para ler. Código vale ~1 min (blockhash). Se a carteira alterar a mensagem ao assinar, a tela recusa: **Solflare recomendada** nos dois READMEs. `rede-local.ts` agora cria a Indústria Demo e o lote #1 vendido. Testado de ponta a ponta com dois navegadores (câmera falsa com Y4M do QR).
- Sessão 12 (cont.): **Repositórios no GitHub.** Blockchain: https://github.com/ECOLchain/ecolchain-core-chains (`main`; programas, clientes, docs). Web: https://github.com/ECOLchain/ecolchain-core-web (`main`), clonado em `web/` (no `.gitignore` daqui), com o histórico da interface extraído por `git subtree split` (10 commits). Os dois juntaram o commit inicial criado pelo GitHub. A web depende de `../onchain/clients/src` (alias `@clientes`); pacote npm ou submódulo fica para quando alguém precisar clonar só a web. Credenciais pelo Git Credential Manager do Windows (configurado por repositório). README da blockchain atualizado com a arquitetura em camadas e a ADR 0010.
- Sessão 12 (cont.): **Minha carteira também para o transportador** — a tela passou para `web/src/paginas/MinhaCarteira.tsx` (comum), primeiro item dos menus Coletor e Transportador, com o texto de uso conforme o papel (coletor: entrega; transportador: retirada do lote, que ele assina junto com a cooperativa). `SoPapel` aceita lista de papéis. `rede-local.ts` cadastra o "Transportador Demo" e grava a semente dele. O leitor de QR na tela de Retiradas fica para quando ela for implementada.
- Sessão 12 (cont.): **Leitor de QR na cooperativa** — botão "Ler QR" ao lado do coletor em "Novo lote de origem": `web/src/componentes/LeitorQr.tsx` (câmera traseira via `getUserMedia`, `jsqr` 1.4.0 a cada 150 ms, quadros reduzidos a 640 px, desliga a câmera ao ler). Seleciona o coletor ativo; mensagens para QR que não é carteira, carteira sem cadastro, de outro papel ou coletor inativo; sem câmera, orienta a escolher na lista. A câmera exige contexto seguro: localhost ou **https** (no celular pela rede local, `http://IP` não abre a câmera). Testado com câmera falsa do Chromium (`--use-file-for-fake-video-capture` com um Y4M do QR da tela Minha carteira).
- Sessão 12 (cont.): **Minha carteira (coletor)** — `web/src/paginas/coletor/MinhaCarteira.tsx`, rota `/minha-carteira`, primeiro item do menu Coletor: nome do participante, QR Code (`qrcode.react` 4.2.0, fundo branco também no tema escuro) com só o endereço da carteira, endereço em grupos de 4 e botão copiar.
- Sessão 12 (cont.): **Meus lotes (coletor)** — `web/src/paginas/coletor/MeusLotes.tsx`, rota `/meus-lotes`, no menu do papel coletor: grade com os lotes de origem da carteira (`origem_ref` = `coletorRef`, em qualquer cooperativa), cooperativa, material, peso, lote de venda (lidos em lotes de 100 com `fetchAllMaybeLote`), situação e link para a trilha; filtros por material e situação. `Grade` ganhou `larguraMinima` (rolagem horizontal no celular). `rede-local.ts` grava a semente do coletor 1 (`coletor` no JSON) para testar como coletor. Testado no navegador (rede local, claro/escuro, celular).
- Sessão 12 (cont.): **Trilha pública com busca por nome** (sem mudança on-chain): nome do participante (sem acentos, espaços normalizados) ou carteira → lotes do coletor (`origem_ref` = `coletorRef`), da cooperativa (offset 8 em `Entrega`/`Lote`), da indústria ou do transportador (offsets fixos de `Lote`, antes do `estado`); os cartões mostram o coletor. O lote de venda aceita **nota fiscal ou recibo** (`evidencias_hash` = `origemRef(texto)`, mesmo hash das origens; sem texto, o hash antigo `lote:<coop>:<id>`), pesquisável na trilha (offset 110 de `Lote`). Resultados sem repetição, até 20 trilhas. Testado na devnet ("Coletor 1" → lotes de origem #1 e #3); o RPC público da devnet devolve 429 em buscas maiores (usar RPC próprio). Em aberto, se o usuário pedir: NF também no lote de origem com coletor (exige campo novo no `ecol_lote` — só 31 bytes livres em `Entrega._reserved` — e upgrade pelo Squads).
- Sessão 12 (cont.): **ADR 0010 implementada.** `ecol_lote`: `Entrega.origem` + `origem_ref`; lote de venda `EmMontagem` → `cooperativa_fechar_lote` (pesagem do consolidado na faixa −10%/+2%, `admin_set_tolerancia_montagem`, Core asset com `PermanentBurnDelegate`) → `Criado`; `cooperativa_desmontar_lote` (`EmDesmontagem` → `Desfeito`, queima o asset); `peso_sem_coletor_g`. `ecol_credito`: `peso_sem_coletor_g` proporcional ao peso consolidado. `ecol_reparticao`: teto dos coletores só sobre o peso com coletor. 129 testes Rust, 6 TS, 2 E2E. Interface: Lotes de origem (origem + referência), Lotes de venda (montar/fechar/desfazer, faixa de peso), Trilha pública sem carteira (busca pela referência normalizada: `origemRef` = sha256("ECOLCHAIN:ORIGEM:v1" ‖ texto em maiúsculas, espaços únicos)). Testado no navegador com rede local. **Devnet:** `ecol_credito` (proposta #15, hash `4b73d878…ee67332`) e `ecol_reparticao` (#16, hash `0fe26d24…5ae5d62f`) atualizados; `ecol_lote` (#17, hash `9d99a79e…2b032efc01`, programa ampliado em 51.248 bytes). Os três programas da ADR 0010 estão na devnet. O criador das propostas (`ecolchain-diretoria`) paga o rent de cada proposta e precisa de saldo.
- Sessão 12 (cont.): planejamento do início do processo com o usuário → **ADR 0010 (aceita)**: lote de origem com ou sem coletor e união em lote de venda.
- Sessão 12 (cont.): **grades e popups** na interface, seguindo o layout do painel central do ErgoRe (`fronts/apps/ergore`, `/cadastros/cnae`): título no subheader do painel, grade com busca/filtros/ordenação e 7 por página, inclusões e alterações em popup (Participantes, Balanças, Materiais, Entregas de coletores, Meus lotes). **Nome da balança on-chain** (`Balanca.nome`, mesmo padrão do participante; `operador_set_balanca_nome`; `codificar_nome` comum em `state.rs`). "Dono" virou "Proprietário" na interface. Upgrade do `ecol_lote` na devnet pela proposta Squads #14 (hash `9ef99030…3347a7a8`); balanças existentes ficam sem nome até o operador preencher. 118 testes Rust + 5 TS + 2 E2E. Testado no navegador (Playwright + carteira Wallet Standard de teste injetada, rede local com 23 participantes e 12 entregas): cadastro, edição, ativação, entrega, montagem e anúncio de lote.
- Sessão 12: **nome do participante on-chain** (`Participante.nome: [u8; 32]`, UTF-8 com zeros, no lugar do `_reserved`, sem mudar o tamanho da conta; cadastros antigos ficam sem nome). `operador_register_participante` exige o nome; nova `operador_set_participante_nome` preenche ou corrige. Interface: coluna Nome em Participantes (clique para renomear), dropdowns de Balanças e Coletas e grades mostram "Nome — carteira". O nome é público: para coletores, apelido ou código (LGPD). O papel continua imutável e não há remoção de participante (pedidos do usuário recusados pelo desenho; alternativa: `operador_close_participante` num upgrade futuro). Transferido 0,5 SOL (devnet) para a Cooperativa 1 (`kv3D…Jg3`), cujo `kyc_hash` = sha256("Cooperativa 1"). Upgrade do `ecol_lote` na devnet pela proposta Squads #13 (build verificável, hash `66154137…e719212`, programa ampliado em 10.240 bytes). 115 testes Rust + 5 TS + 2 E2E. Os participantes existentes ficam com nome vazio até o operador preencher na interface.
- Sessão 11: telas da cooperativa na interface: **Entregas de coletores** (coletor cadastrado, material, peso; balança de teste no navegador assina) e **Meus lotes** (montar com entregas, anunciar, cancelar). `coletor_ref = sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira)` (`clients/src/coletor.ts`). Script `clients/scripts/rede-local.ts` (Surfpool de demonstração); fluxo testado no navegador com carteira que assina de verdade: 9 entregas, lote montado, anúncio.
- Sessão 10: **entregas dos coletores** on-chain (`Entrega`, `cooperativa_register_entrega` com pesagem Ed25519 tipo 2, `cooperativa_add_entregas` para montar o lote; lote só vai a leilão com entregas; `coletor_ref` saiu do lote; repartição dos coletores pelo peso das entregas, R1 reduzido). Upgrades na devnet por Squads (#5–#12, timelock já em 0); ampliação de programa fora da proposta (CPI proibida). 111 testes Rust, 5 TS, 2 E2E. Lances continuam off-chain (decisão do usuário). Guardião nos 3 programas (`admin_set_guardiao`, `guardiao_pausar`, só restringe); Squads do guardião 1 de 5 com os mesmos membros, sem timelock (`ops/guardiao-devnet.ts`, `pausar.ts`, `retomar.ts`); 103 testes Rust. Governança revista (ADR 0009): multisig 3 de 5 só com chaves da EcolChain, timelock 0 na devnet (proposta #3) e 24 h na mainnet; chaves de membro renomeadas para `ecolchain-*`. Simplificação da administração. Materiais viraram cadastro on-chain (`Material`, PDA `["material", codigo]`, `operador_create_material`/`operador_update_material`; lotes e créditos guardam o código u16). Operador da devnet passou para a carteira Solflare do usuário (`EhY5…ec8X`) por proposta Squads; upgrade do `ecol_lote` e `ecol_credito` por proposta Squads (com ampliação da conta). Menu Administração na interface com Participantes, Balanças e Materiais funcionais. Scripts `ops/proposta.ts`, `trocar-operador.ts`, `propor-upgrade.ts`. 96 testes Rust + 4 TS + 2 E2E.
- Sessão 9: interface web iniciada em `web/` (React 19 + Tailwind 4 + Vite): header fixo com logo provisório, fonte P/M/G, tema claro/escuro, idiomas pt-BR/en-US/es-ES, seletor devnet/localnet, carteira via Wallet Standard; menu lateral recolhível com opções por papel da carteira (lidas on-chain) e desabilitadas sem carteira; painel ainda a definir.
- Sessão 8: README.md na raiz; deploy devnet verificável dos 3 programas + Squads 3 de 5 + configs inicializadas.
- Sessão 7: F5 parcial — clientes Codama, gerador TS da árvore, E2E Surfpool, testes de propriedade; scripts de deploy devnet (verificável + Squads 3 de 5) prontos, aguardando Docker e SOL do usuário.
- Sessão 6: F4 concluída (`ecol_reparticao`); `Cenario` do `ecol_credito` extraído para `tests/cenario/mod.rs` e reutilizado; workspace com 84 testes.
- Sessão 5: F3 concluída (`ecol_credito`); decisões C1 (desagregar com prazo) e C2 (intermediador atesta venda) implementadas; workspace com 67 testes.
- Sessão 4: F2 concluída (balança Ed25519, Core asset); auditoria com checklists do solana-ai-kit, 10 achados corrigidos com testes de regressão; custo real medido.
- Sessão 3: plano dos programs aprovado; F0 (workspace `onchain/`, ADRs, git init) e F1 (`ecol_lote` com máquina de estados, 18 testes) concluídas.
- Sessão 2: gerado `docs/Proposta_04.excalidraw` (novo quadro ✅ Decisões, premissas em aberto revisadas, tokens → R$, Core asset, coletor R$ na entrega, Zupy resgata direitos, multisig, custos revisados).
- Sessão 2: fechadas 8 premissas (escrow R$, sem token próprio, Core asset, carteira embutida/MPC, crédito só referência, coletor R$ + cupons, direito on-chain com Zupy resgatando, multisig com terceiros).
