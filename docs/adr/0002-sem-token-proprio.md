# ADR 0002 — Sem token próprio da EcolChain

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
O diagrama original previa a parte da EcolChain em tokens.

## Decisão
A EcolChain recebe sua parte em R$ / stablecoin. Valores on-chain são registros em centavos de R$ (`u64`).

## Consequências
Evita discussão de valor mobiliário (CVM), gestão de mint authority e conversão R$→token.
