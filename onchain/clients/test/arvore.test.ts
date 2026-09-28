import assert from 'node:assert/strict';
import { test } from 'node:test';
import { type Address, generateKeyPairSigner } from '@solana/kit';
import { calcularTotais, folha, type LoteDoCredito, montarArvore, montarDireitos, PapelDireito } from '../src/arvore';

const BPS = { coletor: 500, cooperativa: 1_000, transportador: 500 };
const novo = async () => (await generateKeyPairSigner()).address;

test('totais batem com o exemplo do diagrama (US$ 50)', () => {
    const { totais, ecolchain } = calcularTotais(5_000n, BPS, 6_500);
    assert.deepEqual(totais, [250n, 500n, 250n, 3_250n]);
    assert.equal(ecolchain, 750n);
});

test('divisão por peso fecha o total de cada papel e é determinística', async () => {
    const [c1, c2, c3, coop, t, ind1, ind2] = await Promise.all(Array.from({ length: 7 }, novo));
    const lotes: LoteDoCredito[] = [
        { cooperativa: coop, transportador: t, industria: ind1, pesoG: 333_333n, entregas: [{ coletor: c1, pesoG: 333_333n }] },
        { cooperativa: coop, transportador: t, industria: ind2, pesoG: 333_333n, entregas: [{ coletor: c2, pesoG: 333_333n }] },
        { cooperativa: coop, transportador: t, industria: ind1, pesoG: 333_334n, entregas: [{ coletor: c3, pesoG: 333_334n }] },
        { cooperativa: coop, transportador: t, industria: ind2, pesoG: 1n, entregas: [{ coletor: c1, pesoG: 1n }] },
    ];
    const { totais } = calcularTotais(12_347n, BPS, 7_000);
    const d = montarDireitos(lotes, totais);
    for (const papel of [0, 1, 2, 3] as PapelDireito[]) {
        const soma = d.filter((x) => x.papel === papel).reduce((a, x) => a + x.valorCentavos, 0n);
        assert.equal(soma, totais[papel], `papel ${papel}`);
    }
    assert.deepEqual(d.map((x) => x.indice), d.map((_, i) => i));
    // Mesma entrada em outra ordem → mesma árvore.
    const d2 = montarDireitos([...lotes].reverse(), totais);
    assert.deepEqual(montarArvore(d2).raiz, montarArvore(d).raiz);
});

test('toda folha prova contra a raiz e valor adulterado não prova', async () => {
    const bs: Address[] = await Promise.all(Array.from({ length: 7 }, novo));
    const direitos = bs.map((beneficiario, indice) => ({ indice, beneficiario, papel: PapelDireito.Coletor, valorCentavos: BigInt(indice + 1) }));
    const arvore = montarArvore(direitos);
    direitos.forEach((d, i) => assert.ok(arvore.verificar(d, arvore.prova(i)), `folha ${i}`));
    assert.ok(!arvore.verificar({ ...direitos[0], valorCentavos: 99n }, arvore.prova(0)));
});

test('vetor fixo compartilhado com o Rust (ecol_reparticao::merkle::vetor_compartilhado)', () => {
    const b = 'US517G5965aydkZ46HS38QLi7UQiSojurfbQfKCELFx' as Address; // base58 de [7; 32]
    const h = Buffer.from(folha({ indice: 3, beneficiario: b, papel: PapelDireito.Industria, valorCentavos: 3_250n })).toString('hex');
    assert.equal(h, '0bc16b3d8ce1484b7b16cabbad84e58357203e433e806442ad12b59549280d3c');
});

test('coletores dividem pelo peso das entregas; demais papéis pelo peso do lote', async () => {
    const [c1, c2, coop, t, ind] = (await Promise.all(Array.from({ length: 5 }, () => generateKeyPairSigner()))).map(
        (k) => k.address,
    );
    // Um lote de 1 t formado por duas entregas: 700 kg de c1 e 300 kg de c2 (o fardo pesou 990 kg).
    const lotes: LoteDoCredito[] = [
        { cooperativa: coop, transportador: t, industria: ind, pesoG: 990_000n, entregas: [
            { coletor: c1, pesoG: 700_000n },
            { coletor: c2, pesoG: 300_000n },
        ] },
    ];
    const { totais } = calcularTotais(5_000n, BPS, 6_500);
    const d = montarDireitos(lotes, totais);
    const coletores = d.filter((x) => x.papel === PapelDireito.Coletor);
    assert.deepEqual(
        coletores.map((x) => [x.beneficiario, x.valorCentavos]).sort(),
        [[c1, 175n], [c2, 75n]].sort(),
    );
    assert.equal(d.find((x) => x.papel === PapelDireito.Cooperativa)?.valorCentavos, 500n);
});
