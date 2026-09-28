# ADR 0007 — Direito por participante on-chain; Zupy só resgata

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
Se a Zupy fizesse a divisão, a auditoria pararia nela e ela seria ponto único.

## Decisão
O `ecol_reparticao` grava uma raiz Merkle dos direitos (participante, valor) por crédito vendido. A Zupy registra cada resgate com prova Merkle e recibo.

## Consequências
Se a Zupy sair, os direitos continuam registrados e podem ser pagos por outro meio.
