# ADR 0009 — Governança por multisig 3 de 5 só com chaves da EcolChain

- **Status:** aceita (substitui a [ADR 0008](0008-governanca-multisig-com-terceiros.md))
- **Data:** 2026-09-28

## Contexto
A ADR 0008 previa membros externos no multisig (cooperativas, indústria, auditor). Na operação real haverá dezenas de cooperativas e indústrias: escolher "representantes" seria arbitrário, e coordenar votos entre empresas a cada ajuste travaria a plataforma. Cooperativas e indústrias são clientes da EcolChain, não governantes; a confiança delas vem da transparência (regras e mudanças públicas on-chain, build verificável) e dos contratos off-chain.

Continua necessário proteger as chaves mais poderosas (upgrade dos programas, percentuais, troca de papéis, pausa) contra perda ou roubo de uma chave isolada.

## Decisão
- Upgrade authority, `admin` (vault 0) e `arbitro` (vault 1) continuam num **Squads multisig 3 de 5**, mas **as 5 chaves são da EcolChain**, com pessoas e locais de guarda distintos (ex.: diretoria, tecnologia, operações, financeiro e uma reserva offline).
- **Timelock:** 0 na devnet (mudanças de governança executam na hora, enquanto os programas mudam com frequência); **24 h na mainnet**, para que qualquer mudança aprovada fique pública um dia antes de valer.
- A operação do dia a dia continua com a carteira de **operador** (uma assinatura, sem espera).
- **Guardião:** os três programas ganham o papel `guardiao`, que **só pausa** (`guardiao_pausar`: restringe o modo, nunca libera). O guardião é o vault de um **segundo Squads, 1 de 5, com os mesmos membros e sem timelock**: qualquer pessoa da EcolChain pausa sozinha e na hora. Despausar continua exigindo o admin (3 de 5).

## Consequências
- Nenhuma empresa externa participa da governança; nenhuma pessoa da EcolChain age sozinha (precisa de 3 das 5 chaves).
- Até 2 chaves podem ser perdidas ou comprometidas sem parar nem expor a plataforma.
- Na mainnet, uma mudança de governança leva no mínimo 24 h, mas a **pausa de emergência não espera** (guardião).
- Um guardião comprometido, no pior caso, pausa a plataforma; sem perda de fundos nem de dados, e o admin desfaz.
- O campo `guardiao` ocupa 32 bytes do espaço reservado das configs, sem mudar o tamanho das contas existentes.
- Os programas não mudam: eles só conhecem o admin, que continua sendo o vault do Squads.
