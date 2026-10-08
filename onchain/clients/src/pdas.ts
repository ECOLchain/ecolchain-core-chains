// Endereços derivados (PDAs) usados pelos três programas. Espelham as seeds em Rust.
import {
    type Address,
    getAddressEncoder,
    getProgramDerivedAddress,
    getU16Encoder,
    getU64Encoder,
    type ReadonlyUint8Array,
} from '@solana/kit';
import { ECOL_CREDITO_PROGRAM_ADDRESS } from './generated/ecol_credito';
import { ECOL_LOTE_PROGRAM_ADDRESS } from './generated/ecol_lote';
import { ECOL_REPARTICAO_PROGRAM_ADDRESS } from './generated/ecol_reparticao';

const utf8 = (s: string) => new TextEncoder().encode(s);
const addr = (a: Address) => getAddressEncoder().encode(a);
const u64 = (v: bigint) => getU64Encoder().encode(v);

async function pda(programa: Address, seeds: (Uint8Array | ReadonlyUint8Array)[]): Promise<Address> {
    const [endereco] = await getProgramDerivedAddress({ programAddress: programa, seeds });
    return endereco;
}

/** Conta `__event_authority` do `emit_cpi!` de cada programa. */
export const eventAuthority = (programa: Address) => pda(programa, [utf8('__event_authority')]);

export const lote = {
    config: () => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('global')]),
    participante: (carteira: Address) => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('part'), addr(carteira)]),
    balanca: (dispositivo: Address) => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('balanca'), addr(dispositivo)]),
    entrega: (cooperativa: Address, entregaId: bigint) =>
        pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('entrega'), addr(cooperativa), u64(entregaId)]),
    material: (codigo: number) => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('material'), getU16Encoder().encode(codigo)]),
    lote: (cooperativa: Address, loteId: bigint) =>
        pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('lote'), addr(cooperativa), u64(loteId)]),
    asset: (lotePda: Address) => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('asset'), addr(lotePda)]),
    autoridadeAsset: () => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('autoridade_asset')]),
    /** Variação de um material (índice 1..n), como a cor do vidro (ADR 0011). */
    variacao: (codigo: number, indice: number) =>
        pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('variacao'), getU16Encoder().encode(codigo), Uint8Array.of(indice)]),
    /** Carteira vinculada a um participante (ADR 0011). */
    carteira: (endereco: Address) => pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('carteira'), addr(endereco)]),
    distribuicao: (importador: Address, distribId: bigint) =>
        pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('distrib'), addr(importador), u64(distribId)]),
    coleta: (importador: Address, coletaId: bigint) =>
        pda(ECOL_LOTE_PROGRAM_ADDRESS, [utf8('coleta'), addr(importador), u64(coletaId)]),
};

export const credito = {
    config: () => pda(ECOL_CREDITO_PROGRAM_ADDRESS, [utf8('credito_config')]),
    credito: (creditoId: bigint) => pda(ECOL_CREDITO_PROGRAM_ADDRESS, [utf8('credito'), u64(creditoId)]),
    /** Deve ser a `credito_autoridade` do `GlobalConfig` do `ecol_lote`. */
    autoridade: () => pda(ECOL_CREDITO_PROGRAM_ADDRESS, [utf8('autoridade_credito')]),
    serial: (padrao: number, serialHash: Uint8Array) =>
        pda(ECOL_CREDITO_PROGRAM_ADDRESS, [utf8('serial'), Uint8Array.of(padrao), serialHash]),
};

export const reparticao = {
    config: () => pda(ECOL_REPARTICAO_PROGRAM_ADDRESS, [utf8('rep_config')]),
    distribuicao: (creditoPda: Address) => pda(ECOL_REPARTICAO_PROGRAM_ADDRESS, [utf8('dist'), addr(creditoPda)]),
};
