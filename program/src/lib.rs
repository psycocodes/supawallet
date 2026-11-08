//! SupaWallet: a Solana wallet for every GitHub user, spent with the SSH keys already on their profile.
//!
//! - The **vault** for GitHub ID `n` is the PDA `["vault", n_le]`. It is a plain system account, so anyone can
//!   send SOL to it before the user has ever heard of SupaWallet.
//! - The **key set** PDA `["keys", n_le]` lists the hashes of the user's GitHub SSH keys and when each was first
//!   seen. It is written by the key-sync authority (a GitHub reader run by the operator); a key removed on GitHub is
//!   dropped at the next sync, which revokes it.
//! - **Execute** spends from the vault. The user signs a plain-text intent with stock
//!   `ssh-keygen -Y sign -n supawallet@v1 -O hashalg=sha256` (an ed25519 key). The transaction carries Solana's
//!   native Ed25519 signature check over the SSHSIG signed data; this program rebuilds the intent on-chain,
//!   rebuilds the SSHSIG signed data from it, and checks that it is exactly what the Ed25519 instruction verified,
//!   with a key listed for that GitHub ID. Anyone can submit and earn the signed relayer fee.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    ed25519_program,
    entrypoint,
    entrypoint::ProgramResult,
    hash::hashv,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{instructions as ix_sysvar, Sysvar},
};
use solana_system_interface::instruction as system_ix;

/// The key-sync authority (stand-in for a decentralised oracle).
pub const AUTHORITY: Pubkey = pubkey!("2tRdVcd8B529C4hgcdQZMR9RSPQEKxSK3VBwvzdjLo1L");
pub const NAMESPACE: &[u8] = b"supawallet@v1";
pub const MAX_KEYS: usize = 16;
pub const KEYS_LEN: usize = 1 + MAX_KEYS * 40;
pub const STATE_LEN: usize = 8;
/// Spends above this from a key first seen less than NEW_KEY_DELAY ago are refused (account-takeover guard).
pub const INSTANT_LIMIT: u64 = 500_000_000;
pub const NEW_KEY_DELAY: i64 = 72 * 3600;

entrypoint!(process_instruction);

fn err(code: u32, what: &str) -> ProgramError {
    msg!("supawallet: {}", what);
    ProgramError::Custom(code)
}

fn read_u64(d: &[u8], at: usize) -> Result<u64, ProgramError> {
    d.get(at..at + 8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        .ok_or(ProgramError::InvalidInstructionData)
}

/// The exact text a spend signs. Every line ends with "\n".
pub fn intent(vault: &Pubkey, github_id: u64, nonce: u64, amount: u64, to: &Pubkey, fee: u64) -> String {
    format!(
        "supawallet v1\ncluster: devnet\nvault: {}\ngithub-id: {}\nnonce: {}\nsend: {} lamports\nto: {}\nrelayer-fee: {}\n",
        vault, github_id, nonce, amount, to, fee
    )
}

fn ssh_string(out: &mut Vec<u8>, s: &[u8]) {
    out.extend_from_slice(&(s.len() as u32).to_be_bytes());
    out.extend_from_slice(s);
}

/// OpenSSH PROTOCOL.sshsig: the bytes an SSH key actually signs for `message`.
pub fn sshsig_signed_data(message: &[u8]) -> Vec<u8> {
    let h = hashv(&[message]).to_bytes();
    let mut out = b"SSHSIG".to_vec();
    ssh_string(&mut out, NAMESPACE);
    ssh_string(&mut out, b"");
    ssh_string(&mut out, b"sha256");
    ssh_string(&mut out, &h);
    out
}

/// keyHash = sha256(SSH public key blob), the blob being the base64-decoded second field of an authorized_keys line.
pub fn ed25519_key_hash(pk: &[u8; 32]) -> [u8; 32] {
    let mut blob = Vec::with_capacity(51);
    ssh_string(&mut blob, b"ssh-ed25519");
    ssh_string(&mut blob, pk);
    hashv(&[&blob]).to_bytes()
}

fn find_key(keys: &[u8], hash: &[u8; 32]) -> Option<i64> {
    let n = keys[0] as usize;
    (0..n).find_map(|i| {
        let e = &keys[1 + i * 40..1 + (i + 1) * 40];
        (&e[..32] == hash).then(|| i64::from_le_bytes(e[32..40].try_into().unwrap()))
    })
}

fn create_pda<'a>(
    payer: &AccountInfo<'a>,
    acct: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    seeds: &[&[u8]],
    len: usize,
    program_id: &Pubkey,
) -> ProgramResult {
    if acct.owner == program_id {
        return Ok(());
    }
    let lamports = Rent::get()?.minimum_balance(len).saturating_sub(acct.lamports());
    if lamports > 0 {
        invoke_signed(&system_ix::transfer(payer.key, acct.key, lamports), &[payer.clone(), acct.clone(), system.clone()], &[])?;
    }
    invoke_signed(&system_ix::allocate(acct.key, len as u64), &[acct.clone(), system.clone()], &[seeds])?;
    invoke_signed(&system_ix::assign(acct.key, program_id), &[acct.clone(), system.clone()], &[seeds])?;
    Ok(())
}

pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    match data.first() {
        Some(0) => set_keys(program_id, accounts, &data[1..]),
        Some(1) => execute(program_id, accounts, &data[1..]),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

/// SetKeys: [github_id u64][count u8][count x 32-byte key hash]. The complete current key set from GitHub.
fn set_keys(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let it = &mut accounts.iter();
    let authority = next_account_info(it)?;
    let keys = next_account_info(it)?;
    let state = next_account_info(it)?;
    let system = next_account_info(it)?;
    if !authority.is_signer || *authority.key != AUTHORITY {
        return Err(err(1, "not the key-sync authority"));
    }
    let id = read_u64(data, 0)?;
    let idb = id.to_le_bytes();
    let n = *data.get(8).ok_or(ProgramError::InvalidInstructionData)? as usize;
    if n > MAX_KEYS || data.len() != 9 + n * 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let (kp, kb) = Pubkey::find_program_address(&[b"keys", &idb], program_id);
    let (sp, sb) = Pubkey::find_program_address(&[b"state", &idb], program_id);
    if *keys.key != kp || *state.key != sp {
        return Err(ProgramError::InvalidSeeds);
    }
    create_pda(authority, keys, system, &[b"keys", &idb, &[kb]], KEYS_LEN, program_id)?;
    create_pda(authority, state, system, &[b"state", &idb, &[sb]], STATE_LEN, program_id)?;

    let now = Clock::get()?.unix_timestamp;
    let mut d = keys.try_borrow_mut_data()?;
    let old = d.to_vec();
    let mut fresh = vec![0u8; KEYS_LEN];
    fresh[0] = n as u8;
    for i in 0..n {
        let h: [u8; 32] = data[9 + i * 32..9 + (i + 1) * 32].try_into().unwrap();
        let first_seen = find_key(&old, &h).unwrap_or(now); // keep first-seen for keys still present
        fresh[1 + i * 40..1 + i * 40 + 32].copy_from_slice(&h);
        fresh[1 + i * 40 + 32..1 + (i + 1) * 40].copy_from_slice(&first_seen.to_le_bytes());
    }
    d.copy_from_slice(&fresh); // keys missing from the new set are gone: revoked
    msg!("supawallet: github id {} now has {} key(s)", id, n);
    Ok(())
}

/// Execute: [github_id u64][amount u64][fee u64]. The instruction just before this one must be a native Ed25519
/// verification of the SSHSIG signed data over this spend's intent.
fn execute(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let it = &mut accounts.iter();
    let relayer = next_account_info(it)?;
    let vault = next_account_info(it)?;
    let state = next_account_info(it)?;
    let keys = next_account_info(it)?;
    let to = next_account_info(it)?;
    let sysvar_ix = next_account_info(it)?;
    let system = next_account_info(it)?;

    let id = read_u64(data, 0)?;
    let amount = read_u64(data, 8)?;
    let fee = read_u64(data, 16)?;
    let idb = id.to_le_bytes();
    let (vp, vb) = Pubkey::find_program_address(&[b"vault", &idb], program_id);
    let (sp, _) = Pubkey::find_program_address(&[b"state", &idb], program_id);
    let (kp, _) = Pubkey::find_program_address(&[b"keys", &idb], program_id);
    if *vault.key != vp || *state.key != sp || *keys.key != kp {
        return Err(ProgramError::InvalidSeeds);
    }
    if state.owner != program_id || keys.owner != program_id {
        return Err(err(2, "no keys synced for this GitHub id"));
    }

    // The native Ed25519 check in the previous instruction, with all its data inline.
    let ed = ix_sysvar::get_instruction_relative(-1, sysvar_ix)?;
    if ed.program_id != ed25519_program::id() {
        return Err(err(3, "previous instruction is not an Ed25519 verification"));
    }
    let e = &ed.data;
    if e.len() < 16 || e[0] != 1 {
        return Err(err(4, "expected exactly one Ed25519 signature"));
    }
    let u16_at = |i: usize| u16::from_le_bytes([e[i], e[i + 1]]);
    let (pk_off, msg_off, msg_len) = (u16_at(6) as usize, u16_at(10) as usize, u16_at(12) as usize);
    if u16_at(4) != u16::MAX || u16_at(8) != u16::MAX || u16_at(14) != u16::MAX {
        return Err(err(5, "Ed25519 data must be inline"));
    }
    let pk: [u8; 32] = e.get(pk_off..pk_off + 32).ok_or(ProgramError::InvalidInstructionData)?.try_into().unwrap();
    let signed = e.get(msg_off..msg_off + msg_len).ok_or(ProgramError::InvalidInstructionData)?;

    // Rebuild what the user must have signed.
    let nonce = u64::from_le_bytes(state.try_borrow_data()?[..8].try_into().unwrap());
    let text = intent(vault.key, id, nonce, amount, to.key, fee);
    if signed != sshsig_signed_data(text.as_bytes()).as_slice() {
        return Err(err(6, "signature is not over this spend's intent"));
    }

    // The key must be on the user's GitHub profile, and not brand new for large spends.
    let first_seen = find_key(&keys.try_borrow_data()?, &ed25519_key_hash(&pk))
        .ok_or_else(|| err(7, "key is not on this GitHub profile"))?;
    if amount > INSTANT_LIMIT && Clock::get()?.unix_timestamp < first_seen + NEW_KEY_DELAY {
        return Err(err(8, "new key: large spends wait 72 hours"));
    }

    state.try_borrow_mut_data()?[..8].copy_from_slice(&(nonce + 1).to_le_bytes());
    let seeds: &[&[u8]] = &[b"vault", &idb, &[vb]];
    invoke_signed(&system_ix::transfer(vault.key, to.key, amount), &[vault.clone(), to.clone(), system.clone()], &[seeds])?;
    if fee > 0 {
        invoke_signed(&system_ix::transfer(vault.key, relayer.key, fee), &[vault.clone(), relayer.clone(), system.clone()], &[seeds])?;
    }
    msg!("supawallet: github id {} paid {} lamports to {} (nonce {})", id, amount, to.key, nonce);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_text_is_exact() {
        let v = Pubkey::new_from_array([1; 32]);
        let t = Pubkey::new_from_array([2; 32]);
        let s = intent(&v, 42, 0, 1000, &t, 5);
        assert!(s.starts_with("supawallet v1\ncluster: devnet\nvault: "));
        assert!(s.ends_with("relayer-fee: 5\n"));
        assert_eq!(s.lines().count(), 8);
    }

    #[test]
    fn signed_data_layout() {
        let d = sshsig_signed_data(b"hi");
        assert_eq!(&d[..6], b"SSHSIG");
        assert_eq!(d.len(), 6 + 4 + NAMESPACE.len() + 4 + 4 + 6 + 4 + 32);
    }
}
