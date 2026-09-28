use anchor_lang::prelude::*;

/// De onde veio o material de um lote de origem. Contas antigas (byte zero) são `Coletor`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum OrigemEntrega {
    /// Coletor cadastrado: `origem_ref` identifica o coletor e ele entra na repartição do crédito.
    Coletor,
    /// Coleta ou triagem da própria cooperativa.
    TriagemPropria,
    /// Doação de um gerador (empresa, condomínio, escola, PEV).
    Doacao,
    /// Compra de terceiro (sucateiro, outra cooperativa).
    Compra,
    /// Catador avulso, sem cadastro.
    Avulso,
}

/// Lote de origem (PDA `["entrega", cooperativa, entrega_id]`): material recebido pela cooperativa,
/// pesado e assinado pela balança dela, com ou sem coletor. Entra em no máximo um lote de venda.
#[account]
#[derive(InitSpace)]
pub struct Entrega {
    pub cooperativa: Pubkey,
    pub entrega_id: u64,
    /// Referência da origem como hash (LGPD): o coletor (`sha256("ECOLCHAIN:COLETOR:v1" ‖ carteira)`)
    /// ou o documento da doação / compra / triagem (nota, recibo, CNPJ, ticket). Nunca o texto.
    pub origem_ref: [u8; 32],
    /// Código do material cadastrado.
    pub material: u16,
    pub peso_g: u64,
    /// Balança (PDA) que atestou a pesagem.
    pub balanca: Pubkey,
    pub ts_pesagem: i64,
    /// Lote de venda que recebeu a entrega; `Pubkey::default()` enquanto disponível.
    pub lote: Pubkey,
    pub criado_em: i64,
    pub bump: u8,
    /// Ocupa o primeiro byte do antigo espaço reservado (layout inalterado).
    pub origem: OrigemEntrega,
    pub _reserved: [u8; 31],
}

impl Entrega {
    pub fn com_coletor(&self) -> bool {
        self.origem == OrigemEntrega::Coletor
    }
}
