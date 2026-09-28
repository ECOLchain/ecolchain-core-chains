// Proposta no Squads principal (3 de 5): tira a pausa dos três programas (ecol_lote volta ao Normal).
// Uso: npx tsx retomar.ts   → depois: npx tsx proposta.ts executar <índice> (após o timelock)
import { address, createNoopSigner } from '@solana/kit';
import * as credito from '../clients/src/generated/ecol_credito';
import * as lote from '../clients/src/generated/ecol_lote';
import * as rep from '../clients/src/generated/ecol_reparticao';
import { eventAuthority } from '../clients/src/pdas';
import { paraWeb3, propor, vaultAdmin } from './proposta';

const admin = createNoopSigner(address(vaultAdmin.toBase58()));
const instrucoes = [
    await lote.getAdminSetModoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        modo: lote.ModoOperacao.Normal,
    }),
    await credito.getAdminSetPausadoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        pausado: false,
    }),
    await rep.getAdminSetPausadoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
        pausado: false,
    }),
].map(paraWeb3);

const indice = await propor('retomar operação (tirar pausa)', instrucoes);
console.log(`Proposta #${indice} criada e aprovada por 3 de 5.`);
console.log(`Executar: npx tsx proposta.ts executar ${indice}`);
