# ADR 0012 — Venda direta (preço fixo), lote vendido fora da plataforma e modo de retirada

- **Status:** aceita e implementada (2026-10-07)
- **Data:** 2026-10-07

## Contexto
O usuário pediu duas situações para os lotes de venda da cooperativa:
- **A Vender**, com o valor do lote;
- **Vendido**, sem valor.

Os lotes A Vender aparecem na página **Disputa** da Cooperativa e da Indústria. A Indústria compra informando o **valor** e o **modo de retirada**: transportador terceiro ou transportador próprio. Depois da compra, o lote sai da Disputa e vai para **Compras** (Indústria) e **Vendas** (Cooperativa, página nova).

Decisões do usuário (2026-10-07):
- **A página.** "Disputa" é o antigo Marketplace, agora visível também para a Cooperativa e a Clean Tech. A página de disputas de peso, prazo e entrega passa a se chamar **Contestações**.
- **O valor.** A Indústria paga o preço pedido ou mais, e quem compra primeiro leva.
- **As assinaturas.** Só a Indústria assina a compra. O pagamento e o escrow ficam fora da blockchain. O leilão de três assinaturas continua existindo.
- **"Vendido".** É só uma marca: não tem comprador, valor nem logística na plataforma.

## Decisão

### Layout
Os campos novos do `Lote` ocupam o `_reserved`, que passa de 50 para 48 bytes:
- `venda_direta: bool`;
- `modo_retirada: ModoRetirada`, com os valores `Indefinido` (leilão e contas antigas), `Terceiro` e `Propria`.

`EstadoLote` ganha `VendidoFora` no fim do enum.

### Estados
```
Criado | SemLance ──cooperativa_anunciar_venda(preço)──► Anunciado{prazo = i64::MAX} + venda_direta
Anunciado (venda direta) ──industria_comprar_lote(valor ≥ preço, modo)──► Vendido
Vendido → EmTransporte → Recebido ──cooperativa_confirmar_pagamento(hash do comprovante)──► Reciclado
Criado | SemLance ──cooperativa_set_vendido_fora(true)──► VendidoFora ──(false)──► SemLance
```
- A venda direta reaproveita `Anunciado`, sem prazo. O operador não a encerra como "sem lance".
- O cancelamento é o `cooperativa_cancel_anuncio` de sempre, que também zera `venda_direta`.
- Para mudar o preço, ou trocar entre "à venda" e "vendido", a interface manda as duas instruções numa só transação: primeiro volta a `SemLance`, depois aplica a nova situação.
- `industria_accept_venda` (leilão) recusa lote em venda direta (`LoteEmVendaDireta`), e `industria_comprar_lote` recusa lote de leilão (`LoteNaoEhVendaDireta`).
- A compra direta grava `industria`, `valor_centavos`, `modo_retirada`, `prazo_entrega` e `tolerancia_peso_bps`. Emite `LoteVendido` com `escrow_ref_hash` e `lances_root` zerados.
- **Retirada própria.** Em `transportador_pickup_lote`, quem assina como transportador tem de ser a própria indústria compradora, com papel Indústria (`RetiradaDeOutroModo`). Nos demais modos, continua sendo um transportador.
- **Pagamento.** Sem intermediador, quem fecha a venda direta é a cooperativa: depois do recebimento, ela confirma o pagamento com o hash do comprovante (Pix, boleto ou NF). O lote vai a `Reciclado` e o programa emite `EscrowMovimentado::Liberacao` com o hash.
- **Importador.** A NF de reciclagem do importador também é aceita quando o lote de venda está `VendidoFora`, porque a reciclagem acontece fora da plataforma.

## Consequências
- Instruções novas no `ecol_lote`, que passa a ter 47:
  - `cooperativa_anunciar_venda`;
  - `cooperativa_set_vendido_fora`;
  - `cooperativa_confirmar_pagamento`;
  - `industria_comprar_lote`.
  
  Erros novos: `LoteNaoEhVendaDireta`, `LoteEmVendaDireta`, `ModoRetiradaInvalido` e `RetiradaDeOutroModo`. A devnet precisa de upgrade.
- Mudanças na interface:
  - **Lotes de venda:** a ação "Venda" põe o lote à venda, muda o valor ou marca como vendido;
  - **Disputa:** página nova para Cooperativa, Clean Tech e Indústria, com a compra;
  - **Vendas:** página nova da Cooperativa, com a confirmação do pagamento;
  - **Compras:** ganha a coluna de retirada;
  - **Retiradas:** a Indústria assina a retirada própria.
- **Riscos em aberto:**
  - **Pagamento fora da blockchain.** A cooperativa confirma o pagamento sozinha. Se a indústria não pagar, a cooperativa não confirma e pode abrir contestação. Não há garantia on-chain como no escrow do leilão.
  - **Corrida entre indústrias.** Duas indústrias podem tentar comprar o mesmo lote ao mesmo tempo. Só a primeira transação passa; a segunda recebe `TransicaoInvalida`.
  - **Lote vendido fora sem continuidade.** Não gera crédito de carbono, porque não chega a `Reciclado`, nem repartição pela venda.
