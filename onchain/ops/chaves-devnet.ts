// Gera (ou reaproveita) as chaves de teste da devnet e lista os endereços.
import { carregarOuGerar, DIR, MEMBROS, PAPEIS } from './comum';

for (const nome of [...PAPEIS, ...MEMBROS]) {
    console.log(`${nome.padEnd(22)} ${carregarOuGerar(nome).publicKey.toBase58()}`);
}
console.log(`\nchaves em ${DIR} (privadas; fora do git)`);
