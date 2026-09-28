use anchor_lang::prelude::*;

#[error_code]
pub enum EcolReparticaoError {
    #[msg("Programa pausado")]
    ProgramaPausado,
    #[msg("Somente a upgrade authority do programa pode inicializar a configuração")]
    NaoEhUpgradeAuthority,
    #[msg("Assinante não é o admin")]
    NaoEhAdmin,
    #[msg("Assinante não é o operador")]
    NaoEhOperador,
    #[msg("Assinante não é a Zupy")]
    NaoEhZupy,
    #[msg("Assinante não é o intermediador de pagamento")]
    NaoEhIntermediador,
    #[msg("Chaves de papéis do config devem ser distintas e não nulas")]
    ChavesDeConfigInvalidas,
    #[msg("Nenhuma troca de admin pendente para este assinante")]
    SemAdminPendente,
    #[msg("Percentuais inválidos")]
    PercentuaisInvalidos,
    #[msg("Percentual da indústria fora da faixa negociável")]
    IndustriaForaDaFaixa,
    #[msg("Crédito ainda não vendido")]
    CreditoNaoVendido,
    #[msg("Número de folhas inválido")]
    NumFolhasInvalido,
    #[msg("Distribuição não está no estado exigido")]
    EstadoInvalido,
    #[msg("Índice de folha inválido")]
    IndiceInvalido,
    #[msg("Direito já resgatado")]
    JaResgatado,
    #[msg("Prova Merkle inválida")]
    ProvaInvalida,
    #[msg("Resgate excederia o teto do papel")]
    TetoDoPapelExcedido,
    #[msg("Beneficiário não tem o papel da folha")]
    PapelIncorreto,
    #[msg("Referência (hash) obrigatória não informada")]
    ReferenciaVazia,
    #[msg("Estouro aritmético")]
    Overflow,
    #[msg("Assinante não é o guardião")]
    NaoEhGuardiao,
}
