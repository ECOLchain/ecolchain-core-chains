# ADR 0008 — Governança por multisig Squads com terceiros + timelock

- **Status:** aceita
- **Data:** 2026-09-28

## Contexto
A EcolChain acumula papéis e também recebe na repartição.

## Decisão
Upgrade authority, `admin` e `arbitro` são Squads multisig (ex.: 3 de 5: EcolChain, cooperativas, indústria, auditor) com timelock. `admin_init_global` exige a upgrade authority para evitar front-running na inicialização.

## Consequências
Mudanças são mais lentas. Membros, quórum e prazo do timelock ainda precisam ser definidos.
