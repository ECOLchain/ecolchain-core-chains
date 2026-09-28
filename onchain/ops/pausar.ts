// EMERGÊNCIA: pausa os três programas pelo Squads do guardião (1 de 5, sem timelock).
// Qualquer um dos 5 membros da EcolChain executa sozinho, na hora. Despausar exige o admin (3 de 5).
//
// Uso: npx tsx pausar.ts [membro]      (padrão: o primeiro membro; ex.: ecolchain-tecnologia)
//      npx tsx pausar.ts [membro] --somente-disputas   (ecol_lote em SomenteDisputas em vez de Pausado)
import { address, createNoopSigner } from '@solana/kit';
import * as multisig from '@sqds/multisig';
import { PublicKey, TransactionMessage } from '@solana/web3.js';
import * as credito from '../clients/src/generated/ecol_credito';
import * as lote from '../clients/src/generated/ecol_lote';
import * as rep from '../clients/src/generated/ecol_reparticao';
import { eventAuthority } from '../clients/src/pdas';
import { carregarOuGerar, carteiraLocal, lerJson, MEMBROS } from './comum';
import type { Guardiao } from './guardiao-devnet';
import { connection, paraWeb3 } from './proposta';

const args = process.argv.slice(2);
const nomeMembro = args.find((a) => !a.startsWith('--')) ?? MEMBROS[0];
if (!(MEMBROS as readonly string[]).includes(nomeMembro)) {
    console.error(`membro desconhecido: ${nomeMembro} (use ${MEMBROS.join(', ')})`);
    process.exit(1);
}
const modo = args.includes('--somente-disputas') ? lote.ModoOperacao.SomenteDisputas : lote.ModoOperacao.Pausado;

const g = lerJson<Guardiao>('guardiao.json');
const multisigPda = new PublicKey(g.multisig);
const vault = new PublicKey(g.vault);
const membro = carregarOuGerar(nomeMembro);
const pagador = carteiraLocal();
const guardiao = createNoopSigner(address(g.vault));

const instrucoes = [
    await lote.getGuardiaoPausarInstructionAsync({
        guardiao,
        eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        modo,
    }),
    await credito.getGuardiaoPausarInstructionAsync({
        guardiao,
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
    }),
    await rep.getGuardiaoPausarInstructionAsync({
        guardiao,
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
    }),
].map(paraWeb3);

const conta = await multisig.accounts.Multisig.fromAccountAddress(connection, multisigPda);
const indice = BigInt(conta.transactionIndex.toString()) + 1n;
const { blockhash } = await connection.getLatestBlockhash();
const confirmar = async (sig: Promise<string>) => connection.confirmTransaction(await sig, 'confirmed');

console.log(`Pausando a EcolChain como ${nomeMembro} (${membro.publicKey.toBase58()})…`);
await confirmar(
    multisig.rpc.vaultTransactionCreate({
        connection,
        feePayer: pagador,
        multisigPda,
        transactionIndex: indice,
        creator: membro.publicKey,
        rentPayer: pagador.publicKey,
        vaultIndex: 0,
        ephemeralSigners: 0,
        transactionMessage: new TransactionMessage({ payerKey: vault, recentBlockhash: blockhash, instructions: instrucoes }),
        memo: `pausa de emergência por ${nomeMembro}`,
        signers: [membro],
    }),
);
await confirmar(multisig.rpc.proposalCreate({ connection, feePayer: pagador, creator: membro, multisigPda, transactionIndex: indice }));
await confirmar(multisig.rpc.proposalApprove({ connection, feePayer: pagador, member: membro, multisigPda, transactionIndex: indice }));
await confirmar(
    multisig.rpc.vaultTransactionExecute({
        connection,
        feePayer: pagador,
        multisigPda,
        transactionIndex: indice,
        member: membro.publicKey,
        signers: [membro],
    }),
);
console.log(`EcolChain pausada (ecol_lote em ${lote.ModoOperacao[modo]}; ecol_credito e ecol_reparticao pausados).`);
console.log('Para voltar ao normal: proposta do admin (3 de 5) com admin_set_modo(Normal) e admin_set_pausado(false).');
