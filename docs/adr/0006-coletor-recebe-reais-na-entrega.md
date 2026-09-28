# ADR 0006 — Coletor recebe R$ na entrega + cupons depois

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O crédito leva meses para ser emitido; o coletor não pode esperar.

## Decisão
A cooperativa paga o coletor em R$ (Pix) na entrega, fora da blockchain. Os 5% do crédito chegam depois como direito registrado e resgatado em cupons Zupy.

## Consequências
O pagamento imediato não é rastreado on-chain; o lote guarda só `coletor_ref` (hash com salt).
