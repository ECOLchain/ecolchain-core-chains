// Proposta no Squads principal: define o guardião dos três programas (vault do Squads do guardião).
// Uso: npx tsx definir-guardiao.ts            (usa .devnet/guardiao.json)
//      npx tsx definir-guardiao.ts <endereço>  (outro guardião; 11111111111111111111111111111111 remove)
import { address, createNoopSigner } from '@solana/kit';
import * as credito from '../clients/src/generated/ecol_credito';
import * as lote from '../clients/src/generated/ecol_lote';
import * as rep from '../clients/src/generated/ecol_reparticao';
import { eventAuthority } from '../clients/src/pdas';
import { lerJson } from './comum';
import type { Guardiao } from './guardiao-devnet';
import { paraWeb3, propor, vaultAdmin } from './proposta';

const guardiao = address(process.argv[2] ?? lerJson<Guardiao>('guardiao.json').vault);
const admin = createNoopSigner(address(vaultAdmin.toBase58()));

const instrucoes = [
    await lote.getAdminSetGuardiaoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(lote.ECOL_LOTE_PROGRAM_ADDRESS),
        program: lote.ECOL_LOTE_PROGRAM_ADDRESS,
        guardiao,
    }),
    await credito.getAdminSetGuardiaoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(credito.ECOL_CREDITO_PROGRAM_ADDRESS),
        program: credito.ECOL_CREDITO_PROGRAM_ADDRESS,
        guardiao,
    }),
    await rep.getAdminSetGuardiaoInstructionAsync({
        admin,
        eventAuthority: await eventAuthority(rep.ECOL_REPARTICAO_PROGRAM_ADDRESS),
        program: rep.ECOL_REPARTICAO_PROGRAM_ADDRESS,
        guardiao,
    }),
].map(paraWeb3);

const indice = await propor(`definir guardião ${guardiao}`, instrucoes);
console.log(`Proposta #${indice} criada e aprovada por 3 de 5.`);
console.log(`Executar: npx tsx proposta.ts executar ${indice}`);
