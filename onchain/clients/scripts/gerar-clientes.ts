// Gera os clientes TypeScript (@solana/kit) dos três programas a partir das IDLs do Anchor.
// Rodar depois de `anchor build`: `npm run gerar`.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { rootNodeFromAnchor, type AnchorIdl } from '@codama/nodes-from-anchor';
import { renderVisitor } from '@codama/renderers-js';
import { assertIsNode, bottomUpTransformerVisitor, createFromRoot, instructionAccountNode } from 'codama';

const raiz = join(dirname(fileURLToPath(import.meta.url)), '..');
const programas = ['ecol_lote', 'ecol_credito', 'ecol_reparticao'] as const;

for (const programa of programas) {
    const idl = JSON.parse(readFileSync(join(raiz, '..', 'target', 'idl', `${programa}.json`), 'utf8')) as AnchorIdl;
    const codama = createFromRoot(rootNodeFromAnchor(idl));
    // Conta opcional (`Option<...>` no Anchor) só entra quando o chamador a informa. Sem isso, o cliente
    // async derivaria a PDA (ex.: `["carteira", assinante]`) e a mandaria mesmo quando o titular assina
    // sozinho, e o programa recusaria a conta inexistente.
    codama.update(
        bottomUpTransformerVisitor([
            {
                select: '[instructionAccountNode]',
                transform: (node) => {
                    assertIsNode(node, 'instructionAccountNode');
                    return node.isOptional && node.defaultValue ? instructionAccountNode({ ...node, defaultValue: undefined }) : node;
                },
            },
        ]),
    );
    // `raiz` é o pacote (package.json sincronizado com as dependências do código gerado);
    // só `src/generated/<programa>` é apagado e reescrito.
    await codama.accept(
        renderVisitor(raiz, {
            generatedFolder: join('src', 'generated', programa),
            deleteFolderBeforeRendering: true,
            syncPackageJson: true,
            formatCode: true,
        }),
    );
    console.log(`cliente gerado: src/generated/${programa}`);
}
