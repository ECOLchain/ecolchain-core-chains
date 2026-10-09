# ADR 0013 — Lances on-chain no leilão

- **Status:** aceita e implementada (2026-10-09)
- **Data:** 2026-10-09
- **Revê:** a decisão de 2026-09-28 "lances fora da blockchain" (ver `CONTINUATION.md`, seção 4), só para os leilões em que as indústrias dão lance pela plataforma.

## Contexto
A página **Disputa** (ADR 0012) mostrava à Indústria só os lotes em venda direta. O usuário pediu que ela visse também os lotes em leilão e pudesse **dar lance informando o valor**.

Até aqui, os lances ficavam fora da blockchain: a Administração apurava o leilão e registrava a venda em três assinaturas (operador, intermediador e indústria). O programa guardava só a `lances_root` atestada pelo operador. Não havia servidor para receber lances.

Decisão do usuário (2026-10-09), entre "lance assinado fora da blockchain" e "lance on-chain": **lance on-chain**.

## Decisão

### Conta `Lance`
PDA `["lance", lote, industria]`, uma por indústria e lote. Campos:
- `lote`;
- `industria`: a carteira titular, mesmo quando quem assina é uma carteira vinculada (ADR 0011);
- `valor_centavos`;
- `prazo_leilao`: identifica a rodada do leilão;
- `pagador`: quem pagou o rent;
- `criado_em`, `atualizado_em`, `bump` e `_reserved[32]`.

O rent é de ≈ 0,0021 SOL e é recuperável.

### Campos no `Lote`
Os campos novos ocupam o `_reserved`, que passa de 48 para 4 bytes; o tamanho da conta não muda:
- `lance_lider: Pubkey`;
- `maior_lance_centavos: u64`;
- `qtd_lances: u32`, em que cada aumento conta.

Nas contas antigas esses bytes são zero, o que significa "sem lances".

### Instruções
- `industria_dar_lance(valor)`: o primeiro lance cria a conta `Lance`. Pagador separado: a EcolChain paga como fee payer.
- `industria_aumentar_lance(valor)`: regrava a mesma conta. Serve também para entrar numa nova rodada do mesmo lote, já que o `init_if_needed` é proibido.
- `public_close_lance`: crank público. Fecha o lance de uma rodada encerrada (lote vendido, retirado ou reanunciado) e devolve o rent ao `pagador`. Recusa com `LanceEmVigor` enquanto o lote está anunciado com o mesmo prazo do lance.

### Regras do lance
O lance é aceito só quando todas valem:
- **Assinatura:** só a indústria assina, como titular ou por carteira vinculada. Precisa ter papel Indústria ativo e não pode ser chave de papel do config.
- **Lote em leilão:** `Anunciado` e sem `venda_direta`. Caso contrário, os erros são `LoteNaoEstaEmLeilao` e `LoteEmVendaDireta`.
- **Prazo:** antes do prazo do leilão (`LeilaoEncerrado`).
- **Valor:** pelo menos o preço mínimo (`ValorAbaixoDoMinimo`) e **maior que o maior lance** da rodada (`LanceAbaixoDoMaior`). É um leilão aberto e ascendente: a indústria que dá o lance passa a liderar.
- **Evento:** emite `LanceDado { lote, seq, industria, valor_centavos, prazo_leilao, ts }`.

### Efeitos no leilão
- **Rodadas.** `cooperativa_list_lote` (novo anúncio) e `cooperativa_anunciar_venda` zeram os lances do lote.
- **Retirada pela cooperativa.** `cooperativa_cancel_anuncio` com lances só é aceito depois do prazo (`LeilaoComLances`). Depois do prazo, a cooperativa pode retirar o lote se o vencedor desistir, e os lances são zerados.
- **Sem encerramento "sem lance".** `operador_close_leilao_sem_lance` recusa leilão com lances (`LeilaoComLances`).
- **Venda.** `industria_accept_venda` continua com as três assinaturas. Com `qtd_lances > 0`, exige:
  - o prazo vencido (`LeilaoEmAndamento`);
  - `industria == lance_lider` e `valor == maior_lance_centavos` (`NaoEhLanceVencedor`).
- **Sem lances on-chain.** Com `qtd_lances == 0`, vale a apuração fora da blockchain de antes. É o caso de um leilão presencial com ata.

## Consequências
- O `ecol_lote` passa a ter 50 instruções. São 6 erros novos e 7 testes novos em `tests/lances.rs`.
- **A devnet precisa de upgrade.** A conta `Lote` mantém o tamanho, e os dados antigos decodificam com o layout novo.
- Mudanças na interface:
  - **Disputa:** a Indústria vê leilões e vendas diretas, com o tipo, o maior lance e a marca "Você lidera". "Dar lance" pede o valor e assina; a primeira vez cria a conta do lance, e as seguintes aumentam o lance.
  - **Leilões (operador):** coluna do maior lance. "Registrar venda" fica bloqueado até o prazo quando há lances, e depois vem com a vencedora e o valor preenchidos e travados.
- **Riscos em aberto:**
  - **Lances públicos.** Todos veem o maior lance e quem lidera. É um leilão aberto, não selado.
  - **Lance tardio.** Não há prorrogação automática quando um lance chega perto do prazo.
  - **Vencedor que desiste.** Se a indústria vencedora não assinar a venda ou não depositar o escrow, a cooperativa retira o lote depois do prazo e reanuncia. Não há penalidade on-chain.
  - **Rent dos lances.** Pago pela EcolChain como fee payer; volta pelo crank `public_close_lance`, que precisa ser executado.
