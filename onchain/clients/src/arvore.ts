// Árvore de direitos da repartição. Espelha `ecol_reparticao::merkle` (folha 0x00, nó 0x01,
// pares ordenados) e define a divisão DENTRO de cada papel de forma determinística, para que
// qualquer auditor reconstrua a mesma raiz a partir dos lotes do crédito.
import { createHash } from 'node:crypto';
import { type Address, getAddressEncoder, getU32Encoder, getU64Encoder } from '@solana/kit';

/** Mesma ordem de `ecol_lote::Papel`. */
export enum PapelDireito {
    Coletor = 0,
    Cooperativa = 1,
    Transportador = 2,
    Industria = 3,
}

export interface BpsFixos {
    coletor: number;
    cooperativa: number;
    transportador: number;
}

/**
 * Lote de origem de um coletor vinculado ao lote de venda, com a carteira resolvida off-chain (a entrega
 * guarda só `origem_ref`). Origens sem coletor (triagem, doação, compra, avulso) não entram aqui.
 */
export interface EntregaDoLote {
    coletor: Address;
    pesoG: bigint;
}

/** Lote agregado ao crédito: peso do lote e as entregas de coletores que o formaram. */
export interface LoteDoCredito {
    cooperativa: Address;
    transportador: Address;
    industria: Address;
    pesoG: bigint;
    entregas: EntregaDoLote[];
}

export interface Direito {
    indice: number;
    beneficiario: Address;
    papel: PapelDireito;
    valorCentavos: bigint;
}

const sha256 = (...partes: Uint8Array[]) => {
    const h = createHash('sha256');
    partes.forEach((p) => h.update(p));
    return new Uint8Array(h.digest());
};

/**
 * Mesma conta do programa: floor(receita × bps / 10.000) por papel; EcolChain fica com o resto.
 * Os coletores recebem só sobre o peso com coletor (`Credito.peso_total_g` e `peso_sem_coletor_g`):
 * floor(receita × bps × (total − sem) / (10.000 × total)). Sem pesos, tudo é de coletores.
 */
export function calcularTotais(
    receitaCentavos: bigint,
    bps: BpsFixos,
    bpsIndustria: number,
    pesos: { totalG: bigint; semColetorG: bigint } = { totalG: 0n, semColetorG: 0n },
) {
    const parcela = (b: number) => (receitaCentavos * BigInt(b)) / 10_000n;
    const { totalG, semColetorG } = pesos;
    const coletores =
        totalG === 0n
            ? parcela(bps.coletor)
            : (receitaCentavos * BigInt(bps.coletor) * (totalG - (semColetorG > totalG ? totalG : semColetorG))) /
              (10_000n * totalG);
    const totais = [coletores, parcela(bps.cooperativa), parcela(bps.transportador), parcela(bpsIndustria)];
    const ecolchain = receitaCentavos - totais.reduce((a, b) => a + b, 0n);
    return { totais, ecolchain };
}

function compararBytes(a: Uint8Array, b: Uint8Array): number {
    for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return a[i] - b[i];
    return 0;
}

/**
 * Divide o total de cada papel entre os beneficiários: coletores pelo peso das **entregas**;
 * cooperativa, transportador e indústria pelo peso dos **lotes**. Arredonda para baixo e distribui
 * os centavos restantes pelo maior resto (empate: endereço em ordem crescente). A soma de cada
 * papel é exatamente o seu total. Folhas ordenadas por papel e, dentro do papel, por endereço.
 */
export function montarDireitos(lotes: LoteDoCredito[], totais: bigint[]): Direito[] {
    const enc = getAddressEncoder();
    const pesosDoPapel = (papel: PapelDireito): Map<Address, bigint> => {
        const pesos = new Map<Address, bigint>();
        const somar = (b: Address, p: bigint) => pesos.set(b, (pesos.get(b) ?? 0n) + p);
        for (const l of lotes) {
            if (papel === PapelDireito.Coletor) l.entregas.forEach((e) => somar(e.coletor, e.pesoG));
            else if (papel === PapelDireito.Cooperativa) somar(l.cooperativa, l.pesoG);
            else if (papel === PapelDireito.Transportador) somar(l.transportador, l.pesoG);
            else somar(l.industria, l.pesoG);
        }
        return pesos;
    };
    const direitos: Omit<Direito, 'indice'>[] = [];
    for (const papel of [PapelDireito.Coletor, PapelDireito.Cooperativa, PapelDireito.Transportador, PapelDireito.Industria]) {
        const pesos = pesosDoPapel(papel);
        const pesoTotal = [...pesos.values()].reduce((a, b) => a + b, 0n);
        if (pesoTotal === 0n) throw new Error(papel === PapelDireito.Coletor ? 'crédito sem entregas de coletores' : 'crédito sem peso');
        const total = totais[papel];
        const linhas = [...pesos.entries()]
            .map(([beneficiario, peso]) => ({
                beneficiario,
                base: (total * peso) / pesoTotal,
                resto: (total * peso) % pesoTotal,
            }))
            .sort((a, b) => compararBytes(enc.encode(a.beneficiario) as Uint8Array, enc.encode(b.beneficiario) as Uint8Array));
        let sobra = total - linhas.reduce((a, l) => a + l.base, 0n);
        const porResto = [...linhas].sort((a, b) => (b.resto > a.resto ? 1 : b.resto < a.resto ? -1 : 0));
        for (const l of porResto) {
            if (sobra === 0n) break;
            l.base += 1n;
            sobra -= 1n;
        }
        for (const l of linhas) {
            if (l.base > 0n) direitos.push({ beneficiario: l.beneficiario, papel, valorCentavos: l.base });
        }
    }
    return direitos.map((d, indice) => ({ ...d, indice }));
}

export function folha(d: Direito): Uint8Array {
    return sha256(
        Uint8Array.of(0),
        getU32Encoder().encode(d.indice) as Uint8Array,
        getAddressEncoder().encode(d.beneficiario) as Uint8Array,
        Uint8Array.of(d.papel),
        getU64Encoder().encode(d.valorCentavos) as Uint8Array,
    );
}

export function no(a: Uint8Array, b: Uint8Array): Uint8Array {
    const [menor, maior] = compararBytes(a, b) <= 0 ? [a, b] : [b, a];
    return sha256(Uint8Array.of(1), menor, maior);
}

function subir(nivel: Uint8Array[]): Uint8Array[] {
    const acima: Uint8Array[] = [];
    for (let i = 0; i < nivel.length; i += 2) acima.push(i + 1 < nivel.length ? no(nivel[i], nivel[i + 1]) : nivel[i]);
    return acima;
}

export function montarArvore(direitos: Direito[]) {
    const folhas = direitos.map(folha);
    let nivel = folhas;
    while (nivel.length > 1) nivel = subir(nivel);
    const raiz = nivel[0] ?? new Uint8Array(32);

    const prova = (indice: number): Uint8Array[] => {
        const caminho: Uint8Array[] = [];
        let n = folhas;
        let i = indice;
        while (n.length > 1) {
            const irmao = n[i ^ 1];
            if (irmao) caminho.push(irmao);
            n = subir(n);
            i = Math.floor(i / 2);
        }
        return caminho;
    };
    const verificar = (d: Direito, p: Uint8Array[]) =>
        compararBytes(p.reduce((acc, irmao) => no(acc, irmao), folha(d)), raiz) === 0;

    return { raiz, folhas, prova, verificar };
}
