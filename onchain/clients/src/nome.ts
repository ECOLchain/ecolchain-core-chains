/** Nome de exibição de participantes e balanças: UTF-8 completado com zeros em 32 bytes (todo zero = sem nome). */
export const NOME_FIXO_MAX = 32;

export function lerNomeFixo(campo: ArrayLike<number>): string {
    const bytes = Uint8Array.from(campo);
    const fim = bytes.indexOf(0);
    return new TextDecoder().decode(fim === -1 ? bytes : bytes.subarray(0, fim));
}

/** Tamanho do nome em bytes, como o programa confere. */
export const bytesDoNome = (nome: string) => new TextEncoder().encode(nome).length;
