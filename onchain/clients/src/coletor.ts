// Referência on-chain do coletor (campo `coletor_ref` da entrega): sha256 de um domínio fixo e da
// carteira do coletor. Determinística, então a repartição liga cada entrega à carteira sem tabela
// extra; nenhum dado pessoal vai on-chain (o cadastro do coletor fica off-chain, só o hash do KYC).
import { type Address, getAddressEncoder } from '@solana/kit';

const DOMINIO = new TextEncoder().encode('ECOLCHAIN:COLETOR:v1');

export async function coletorRef(carteira: Address): Promise<Uint8Array> {
    const endereco = getAddressEncoder().encode(carteira) as Uint8Array;
    const dados = new Uint8Array(DOMINIO.length + endereco.length);
    dados.set(DOMINIO, 0);
    dados.set(endereco, DOMINIO.length);
    return new Uint8Array(await crypto.subtle.digest('SHA-256', dados));
}
