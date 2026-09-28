# ADR 0001 — Escrow em R$ off-chain via intermediador

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O pagamento do lote fica retido até o recebimento. Cooperativas e indústrias operam em R$/Pix.

## Decisão
O dinheiro fica com um intermediador de pagamentos (Pix / conta garantia). O `ecol_lote` só registra depósito, liberação e reembolso, cada um co-assinado pela chave `intermediador` do `GlobalConfig`.

## Consequências
Sem cripto para os participantes e menos risco regulatório. Em troca, confia-se no intermediador: a blockchain prova que ele declarou, não que o dinheiro se moveu. Migrar para stablecoin em vault PDA exige um novo programa, não um upgrade simples.
