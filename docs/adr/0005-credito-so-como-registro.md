# ADR 0005 — Crédito de carbono só como registro on-chain

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O crédito nasce no registro oficial (Verra / Gold Standard) e tokenizá-lo cria risco de dupla contagem.

## Decisão
O `ecol_credito` grava nº de série, lotes agregados e prova de aposentadoria. Não há token do crédito.

## Consequências
Sem ponte nem custodiante. A liquidez on-chain do crédito fica fora do escopo.

**Adendo (2026-09-28):** lotes de crédito rejeitado ou abandonado (mais de 180 dias em formação) voltam a `Reciclado` por um crank público e podem integrar outro crédito. O valor da venda do crédito, base da repartição, é co-assinado pelo intermediador de pagamento.
