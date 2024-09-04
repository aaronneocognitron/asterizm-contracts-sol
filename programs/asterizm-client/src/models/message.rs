use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar;

use crate::asterizm_initializer::accounts::InitializerSettings;
use crate::asterizm_initializer::program::AsterizmInitializer;
use crate::asterizm_relayer::accounts::{Chain, RelayerSettings};
use crate::asterizm_relayer::program::AsterizmRelayer;
use crate::{
    asterizm_initializer, ClientAccount, ClientProgramSettings, ClientSender, ClientTrustedAddress,
};

pub const TRANSFER_ACCOUNT_LEN: usize =
  1                                       // success_receive
+ 1                                       // success_execute
+ 1                                       // bump
;

#[account]
#[derive(Default)]
pub struct TransferAccount {
    pub success_receive: bool,
    pub success_execute: bool,
    pub bump: u8,
}

#[event]
pub struct InitiateTransferEvent {
    pub dst_chain_id: u64,
    pub trusted_address: Pubkey,
    pub id: u32,
    pub transfer_hash: [u8; 32],
    pub payload: Vec<u8>,
}

#[derive(Accounts)]
#[instruction(user_address: Pubkey, dst_chain_id: u64)]
pub struct InitSendMessage<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = ["settings".as_bytes()],
        bump = settings_account.bump,
    )]
    pub settings_account: Box<Account<'info, ClientProgramSettings>>,
    #[account(mut,
        seeds = ["client".as_bytes(), &user_address.to_bytes()],
        bump = client_account.bump
    )]
    pub client_account: Box<Account<'info, ClientAccount>>,
    #[account(
        seeds = ["trusted_address".as_bytes(), &user_address.to_bytes(), &dst_chain_id.to_le_bytes()],
        bump = trusted_address.bump,
        constraint = authority.key() == user_address
    )]
    pub trusted_address: Box<Account<'info, ClientTrustedAddress>>,
    #[account(mut)]
    /// CHECK: This is not dangerous because we will create it inside this instruction
    pub transfer_account: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
    #[account(
        seeds = ["chain".as_bytes(), &dst_chain_id.to_le_bytes()],
        bump = chain_account.bump,
        seeds::program = relayer_program.key()
    )]
    pub chain_account: Box<Account<'info, Chain>>,
    pub relayer_program: Program<'info, AsterizmRelayer>,
}

#[derive(Debug)]
pub struct InitMessage {
    pub src_chain_id: u64,
    pub src_address: Pubkey,
    pub dst_chain_id: u64,
    pub dst_address: Pubkey,
    pub tx_id: u32,
    pub payload: Vec<u8>,
}

pub fn serialize_init_message_eth(message: InitMessage) -> Vec<u8> {
    let mut result = vec![];

    let mut word = [0u8; 256];
    word[(32 - 4)..32].copy_from_slice(&32u32.to_be_bytes());
    word[(64 - 8)..64].copy_from_slice(&message.src_chain_id.to_be_bytes());
    word[64..96].copy_from_slice(&message.src_address.to_bytes());
    word[(128 - 8)..128].copy_from_slice(&message.dst_chain_id.to_be_bytes());
    word[128..160].copy_from_slice(&message.dst_address.to_bytes());
    word[(192 - 4)..192].copy_from_slice(&message.tx_id.to_be_bytes());
    word[(224 - 4)..224].copy_from_slice(&192u32.to_be_bytes());
    word[(256 - 4)..256].copy_from_slice(&(message.payload.len() as u32).to_be_bytes());

    result.extend_from_slice(&word);
    result.extend_from_slice(&message.payload);
    result
}

pub fn build_crosschain_hash(_packed: &[u8]) -> [u8; 32] {
    let static_chunk = &_packed[..112];
    let mut hash = anchor_lang::solana_program::hash::hash(&static_chunk);

    let payload_chunk = &_packed[112..];
    let payload_length = payload_chunk.len();
    let chunk_length = 127;
    for i in 0..(payload_length / chunk_length) {
        let from = chunk_length * i;
        let chunk = if from + chunk_length <= payload_length {
            &payload_chunk[from..from + chunk_length]
        } else {
            &payload_chunk[from..payload_length]
        };

        let mut encoded = [0u8; 64];
        encoded[0..32].copy_from_slice(&hash.to_bytes());
        encoded[32..].copy_from_slice(&anchor_lang::solana_program::hash::hash(&chunk).to_bytes());
        hash = anchor_lang::solana_program::hash::hash(&encoded);
    }

    return hash.to_bytes();
}

#[derive(Accounts)]
#[instruction(user_address: Pubkey, dst_chain_id: u64, _tx_id: u32, transfer_hash: [u8; 32],)]
pub struct SendMessage<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut,
        seeds = ["client".as_bytes(), &user_address.to_bytes()],
        bump = client_account.bump
    )]
    pub client_account: Box<Account<'info, ClientAccount>>,
    #[account(
        seeds = ["trusted_address".as_bytes(), &user_address.to_bytes(), &dst_chain_id.to_le_bytes()],
        bump = trusted_address.bump,
    )]
    pub trusted_address: Box<Account<'info, ClientTrustedAddress>>,
    #[account(
        seeds = ["sender".as_bytes(), &user_address.to_bytes(), &sender.address.to_bytes()],
        bump = sender.bump,
        constraint = authority.key() == sender.address
    )]
    pub sender: Box<Account<'info, ClientSender>>,
    #[account(
        mut,
        seeds = ["outgoing_transfer".as_bytes(), &user_address.to_bytes(), &transfer_hash],
        bump = transfer_account.bump
    )]
    pub transfer_account: Box<Account<'info, TransferAccount>>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    pub initializer_settings_account: Account<'info, InitializerSettings>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    pub relayer_settings_account: Account<'info, RelayerSettings>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    #[account(mut)]
    pub system_relay_account_owner: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in relayer
    pub relay_account: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in relayer
    #[account(mut)]
    pub relay_account_owner: Option<AccountInfo<'info>>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in relayer
    pub chain_account: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in relayer
    pub relayer_program: Program<'info, AsterizmRelayer>,
    pub initializer_program: Program<'info, AsterizmInitializer>,
    pub system_program: Program<'info, System>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    pub blocked_src_account: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    pub blocked_dst_account: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: This is not dangerous because we will check it inside the instruction in initializer
    pub initializer_transfer_account: AccountInfo<'info>,
    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

impl<'a, 'b, 'c, 'info> From<&mut SendMessage<'info>>
    for CpiContext<'a, 'b, 'c, 'info, asterizm_initializer::cpi::accounts::SendMessage<'info>>
{
    fn from(
        accounts: &mut SendMessage<'info>,
    ) -> CpiContext<'a, 'b, 'c, 'info, asterizm_initializer::cpi::accounts::SendMessage<'info>>
    {
        let cpi_accounts = asterizm_initializer::cpi::accounts::SendMessage {
            authority: accounts.authority.to_account_info(),
            settings_account: accounts.initializer_settings_account.to_account_info(),
            relayer_settings_account: accounts.relayer_settings_account.to_account_info(),
            relay_account_owner: accounts.relay_account_owner.clone(),
            relay_account: accounts.relay_account.clone(),
            chain_account: accounts.chain_account.clone(),
            relayer_program: accounts.relayer_program.to_account_info(),
            system_program: accounts.system_program.to_account_info(),
            blocked_src_account: accounts.blocked_src_account.clone(),
            blocked_dst_account: accounts.blocked_dst_account.clone(),
            system_relay_account_owner: accounts.system_relay_account_owner.clone(),
            transfer_account: accounts.initializer_transfer_account.clone(),
            instruction_sysvar_account: accounts.instruction_sysvar_account.clone(),
        };
        let cpi_program = accounts.initializer_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

#[derive(Accounts)]
#[instruction(_dst_address: Pubkey, src_address: Pubkey, src_chain_id: u64, _tx_id: u32, transfer_hash: [u8; 32],)]
pub struct InitReceiveMessage<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = ["client".as_bytes(), &_dst_address.to_bytes()],
        bump = client_account.bump,
        constraint = authority.key() == client_account.relay_owner
    )]
    pub client_account: Box<Account<'info, ClientAccount>>,
    #[account(
        seeds = ["trusted_address".as_bytes(), &_dst_address.to_bytes(), &src_chain_id.to_le_bytes()],
        bump = trusted_address.bump,
        constraint = trusted_address.address == src_address
    )]
    pub trusted_address: Box<Account<'info, ClientTrustedAddress>>,
    #[account(
        init,
        payer = authority,
        space = 8 + TRANSFER_ACCOUNT_LEN,
        seeds = ["incoming_transfer".as_bytes(), &_dst_address.to_bytes(), &transfer_hash],
        bump
    )]
    pub transfer_account: Account<'info, TransferAccount>,
    pub system_program: Program<'info, System>,
    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[event]
pub struct PayloadReceivedEvent {
    pub src_chain_id: u64,
    pub src_address: Pubkey,
    pub tx_id: u32,
    pub transfer_hash: [u8; 32],
}

#[derive(Accounts)]
#[instruction(_dst_address: Pubkey, _tx_id: u32, src_chain_id: u64, src_address: Pubkey, transfer_hash: [u8; 32],)]
pub struct ReceiveMessage<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = ["settings".as_bytes()],
        bump = settings_account.bump,
    )]
    pub settings_account: Box<Account<'info, ClientProgramSettings>>,
    #[account(
        seeds = ["client".as_bytes(), &_dst_address.to_bytes()],
        bump = client_account.bump,
    )]
    pub client_account: Box<Account<'info, ClientAccount>>,
    #[account(
        seeds = ["sender".as_bytes(), &_dst_address.to_bytes(), &sender.address.to_bytes()],
        bump = sender.bump,
        constraint = authority.key() == sender.address
    )]
    pub sender: Box<Account<'info, ClientSender>>,
    #[account(
        seeds = ["trusted_address".as_bytes(), &_dst_address.to_bytes(), &src_chain_id.to_le_bytes()],
        bump = trusted_address.bump,
        constraint = trusted_address.address == src_address
    )]
    pub trusted_address: Box<Account<'info, ClientTrustedAddress>>,
    #[account(
        seeds = ["incoming_transfer".as_bytes(), &_dst_address.to_bytes(), &transfer_hash],
        bump = transfer_account.bump
    )]
    pub transfer_account: Account<'info, TransferAccount>,
    #[account(
        seeds = ["chain".as_bytes(), &src_chain_id.to_le_bytes()],
        bump = chain_account.bump,
        seeds::program = relayer_program.key()
    )]
    pub chain_account: Box<Account<'info, Chain>>,
    pub relayer_program: Program<'info, AsterizmRelayer>,
}

#[derive(Accounts)]
pub struct TransferSendingResult<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: account constraints checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[event]
pub struct TransferSendingResultEvent {
    pub dst_address: Pubkey,
    pub transfer_hash: [u8; 32],
    pub status_code: u8,
}