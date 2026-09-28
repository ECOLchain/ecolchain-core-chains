// Referência on-chain da origem de um lote de origem (campo `origem_ref` da entrega).
//
// Coletor: sha256 de um domínio fixo e da carteira do coletor. Determinística, então a repartição
// liga cada entrega à carteira sem tabela extra; nenhum dado pessoal vai on-chain (o cadastro do
// coletor fica off-chain, só o hash do KYC).
import { type Address, getAddressEncoder } from '@solana/kit';

const DOMINIO = new TextEncoder().encode('ECOLCHAIN:COLETOR:v1');

export async function coletorRef(carteira: Address): Promise<Uint8Array> {
    const endereco = getAddressEncoder().encode(carteira) as Uint8Array;
    const dados = new Uint8Array(DOMINIO.length + endereco.length);
    dados.set(DOMINIO, 0);
    dados.set(endereco, DOMINIO.length);
    return new Uint8Array(await crypto.subtle.digest('SHA-256', dados));
}

const DOMINIO_ORIGEM = new TextEncoder().encode('ECOLCHAIN:ORIGEM:v1');

/**
 * Forma canônica do texto da referência (nota, recibo, CNPJ, ticket): sem espaços nas pontas,
 * espaços internos únicos e maiúsculas. Quem digita "nf 123" ou "NF  123 " chega ao mesmo hash.
 */
export const normalizarReferencia = (texto: string) => texto.trim().replace(/\s+/g, ' ').toUpperCase();

/**
 * Referência de origens sem coletor (doação, compra, triagem, avulso): sha256 do domínio e do texto
 * normalizado. O texto nunca vai on-chain; quem o tem (ex.: o doador, com o recibo) recalcula o
 * hash e encontra o lote na trilha pública.
 */
export async function origemRef(texto: string): Promise<Uint8Array> {
    const corpo = new TextEncoder().encode(normalizarReferencia(texto));
    const dados = new Uint8Array(DOMINIO_ORIGEM.length + corpo.length);
    dados.set(DOMINIO_ORIGEM, 0);
    dados.set(corpo, DOMINIO_ORIGEM.length);
    return new Uint8Array(await crypto.subtle.digest('SHA-256', dados));
}
