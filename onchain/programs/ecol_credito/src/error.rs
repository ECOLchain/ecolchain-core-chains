use anchor_lang::prelude::*;

#[error_code]
pub enum EcolCreditoError {
    #[msg("Programa pausado")]
    ProgramaPausado,
    #[msg("Somente a upgrade authority do programa pode inicializar a configuração")]
    NaoEhUpgradeAuthority,
    #[msg("Assinante não é o admin")]
    NaoEhAdmin,
    #[msg("Assinante não é o operador")]
    NaoEhOperador,
    #[msg("Assinante não é o registrador")]
    NaoEhRegistrador,
    #[msg("Chaves de papéis do config devem ser distintas e não nulas")]
    ChavesDeConfigInvalidas,
    #[msg("Nenhuma troca de admin pendente para este assinante")]
    SemAdminPendente,
    #[msg("Transição de estado inválida para o estado atual do crédito")]
    TransicaoInvalida,
    #[msg("Informe entre 1 e 10 lotes por transação")]
    QuantidadeDeLotesInvalida,
    #[msg("Conta de lote não pertence ao programa ecol_lote")]
    LoteDeOutroPrograma,
    #[msg("Material do lote difere do material do crédito")]
    MaterialDiferente,
    #[msg("Lote não ficou agregado a este crédito")]
    AgregacaoNaoConfirmada,
    #[msg("Crédito sem lotes")]
    CreditoVazio,
    #[msg("Número de série vazio ou longo demais")]
    SerialInvalido,
    #[msg("Parâmetro inválido")]
    ParametroInvalido,
    #[msg("Referência (hash) obrigatória não informada")]
    ReferenciaVazia,
    #[msg("Estouro aritmético")]
    Overflow,
    #[msg("Prazo de formação do crédito ainda não expirou")]
    PrazoNaoExpirado,
    #[msg("Lotes só podem ser liberados de crédito rejeitado ou abandonado")]
    LiberacaoNaoPermitida,
    #[msg("Lote não pertence a este crédito")]
    LoteDeOutroCredito,
    #[msg("Assinante não é o intermediador de pagamento")]
    NaoEhIntermediador,
}
