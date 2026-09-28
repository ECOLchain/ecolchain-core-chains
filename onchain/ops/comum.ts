// Chaves e endereços da devnet, em onchain/.devnet (fora do git).
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Keypair } from '@solana/web3.js';

export const DIR = join(dirname(fileURLToPath(import.meta.url)), '..', '.devnet');
export const RPC_DEVNET = process.env.RPC_DEVNET ?? 'https://api.devnet.solana.com';

/** Papéis operacionais (chaves simples) e membros do Squads (3 de 5), todos da EcolChain (ADR 0009). */
export const PAPEIS = ['operador', 'intermediador', 'registrador', 'zupy'] as const;
export const MEMBROS = ['ecolchain-diretoria', 'ecolchain-tecnologia', 'ecolchain-operacoes', 'ecolchain-financeiro', 'ecolchain-reserva'] as const;

export function carregarOuGerar(nome: string): Keypair {
    mkdirSync(DIR, { recursive: true, mode: 0o700 });
    const arquivo = join(DIR, `${nome}.json`);
    if (existsSync(arquivo)) return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(arquivo, 'utf8'))));
    const k = Keypair.generate();
    writeFileSync(arquivo, JSON.stringify(Array.from(k.secretKey)), { mode: 0o600 });
    return k;
}

export function carteiraLocal(): Keypair {
    const arquivo = process.env.CARTEIRA ?? join(homedir(), '.config', 'solana', 'id.json');
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(arquivo, 'utf8'))));
}

export function salvarJson(nome: string, dados: unknown) {
    writeFileSync(join(DIR, nome), JSON.stringify(dados, null, 2));
}

export function lerJson<T>(nome: string): T {
    return JSON.parse(readFileSync(join(DIR, nome), 'utf8')) as T;
}
