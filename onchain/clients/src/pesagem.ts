// Atestação de pesagem: a balança assina (domínio, tipo, lote, peso_g, ts) e a instrução do
// programa Ed25519 vai imediatamente antes da instrução do `ecol_lote`. Espelha `pesagem.rs`.
import {
    type Address,
    AccountRole,
    type Instruction,
    getAddressEncoder,
    getI64Encoder,
    getU64Encoder,
    type KeyPairSigner,
    signBytes,
    address,
} from '@solana/kit';

export const ED25519_PROGRAM_ADDRESS = address('Ed25519SigVerify111111111111111111111111111');

export enum TipoPesagem {
    Origem = 0,
    Recebimento = 1,
}

const DOMINIO = new TextEncoder().encode('ECOLCHAIN:PESO:v1');

export function mensagemPesagem(tipo: TipoPesagem, lote: Address, pesoG: bigint, ts: bigint): Uint8Array {
    const m = new Uint8Array(66);
    m.set(DOMINIO, 0);
    m[17] = tipo;
    m.set(getAddressEncoder().encode(lote), 18);
    m.set(getU64Encoder().encode(pesoG), 50);
    m.set(getI64Encoder().encode(ts), 58);
    return m;
}

/**
 * Instrução Ed25519 com uma assinatura e todos os dados nela mesma (`instruction_index = u16::MAX`),
 * o único formato que o `ecol_lote` aceita.
 */
export async function instrucaoPesagem(
    balanca: KeyPairSigner,
    tipo: TipoPesagem,
    lote: Address,
    pesoG: bigint,
    ts: bigint,
): Promise<Instruction> {
    const msg = mensagemPesagem(tipo, lote, pesoG, ts);
    const assinatura = await signBytes(balanca.keyPair.privateKey, msg);
    const chave = getAddressEncoder().encode(balanca.address);
    const [pkOff, sigOff, msgOff] = [16, 48, 112];
    const cabecalho = new DataView(new ArrayBuffer(16));
    cabecalho.setUint8(0, 1); // uma assinatura
    const u16 = [sigOff, 0xffff, pkOff, 0xffff, msgOff, msg.length, 0xffff];
    u16.forEach((v, i) => cabecalho.setUint16(2 + i * 2, v, true));
    const data = new Uint8Array(msgOff + msg.length);
    data.set(new Uint8Array(cabecalho.buffer), 0);
    data.set(chave, pkOff);
    data.set(assinatura, sigOff);
    data.set(msg, msgOff);
    return { programAddress: ED25519_PROGRAM_ADDRESS, accounts: [], data };
}

/** Anexa contas graváveis (ex.: lotes em `operador_add_lotes`) a uma instrução gerada. */
export function comContasGravaveis<T extends Instruction>(ix: T, contas: Address[]): T {
    return {
        ...ix,
        accounts: [...(ix.accounts ?? []), ...contas.map((a) => ({ address: a, role: AccountRole.WRITABLE }))],
    };
}
