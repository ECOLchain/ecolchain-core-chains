# ADR 0007 — Direito por participante on-chain; Zupy só resgata

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
Se a Zupy fizesse a divisão, a auditoria pararia nela e ela seria ponto único.

## Decisão
O `ecol_reparticao` grava uma raiz Merkle dos direitos (participante, valor) por crédito vendido. A Zupy registra cada resgate com prova Merkle e recibo.

## Consequências
Se a Zupy sair, os direitos continuam registrados e podem ser pagos por outro meio.

**Implementação (2026-09-28):** tetos por papel calculados on-chain a partir do valor de venda do crédito (atestado pelo intermediador) e dos percentuais do config; a árvore Merkle só distribui dentro de cada papel. O intermediador atesta o repasse à Zupy (cupons) e à EcolChain (R$) antes de qualquer resgate. A EcolChain recebe o restante (1% a 15%, mais o resíduo de arredondamento) e não é folha da árvore.
