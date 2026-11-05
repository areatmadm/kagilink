//! KagiLink TPM feasibility probe (D022). READ-ONLY and local:
//! * no persistent keys are created (only one unnamed, ephemeral TPM key that is freed at once),
//! * no network, no files written, no secrets,
//! * every result is a CLAIM reported by the OS or the TPM, not verified trust.
//!
//! It answers: is a TPM 2.0 reachable by a NORMAL user, which commands the TPM broker lets
//! through, whether a TPM-resident key can be made without administrator rights, and whether
//! EK material is readable. It does not and cannot prove the PC is free of malware.

#[cfg(not(windows))]
fn main() {
    println!("kagilink-tpm-probe: Windows only (no TPM check was made).");
}

#[cfg(windows)]
fn main() {
    probe::run();
}

#[cfg(windows)]
mod probe {
    use std::ffi::c_void;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::Cryptography::{
        BCryptBuffer, BCryptBufferDesc, BCryptGenRandom, NCryptCreateClaim,
        NCryptCreatePersistedKey, NCryptDeleteKey, NCryptFinalizeKey, NCryptFreeObject,
        NCryptGetProperty, NCryptOpenStorageProvider, NCryptSetProperty, NCryptVerifyClaim,
        BCRYPTGENRANDOM_FLAGS, BCRYPT_ECDSA_P256_ALGORITHM, BCRYPT_RSA_ALGORITHM, CERT_KEY_SPEC,
        MS_PLATFORM_CRYPTO_PROVIDER, NCRYPTBUFFER_CLAIM_KEYATTESTATION_NONCE,
        NCRYPTBUFFER_TPM_PLATFORM_CLAIM_NONCE, NCRYPTBUFFER_TPM_PLATFORM_CLAIM_PCR_MASK,
        NCRYPT_CLAIM_AUTHORITY_AND_SUBJECT, NCRYPT_CLAIM_PLATFORM, NCRYPT_FLAGS, NCRYPT_HANDLE,
        NCRYPT_KEY_HANDLE, NCRYPT_PCP_IDENTITY_KEY, NCRYPT_PCP_KEY_USAGE_POLICY_PROPERTY,
        NCRYPT_PROV_HANDLE,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::System::TpmBaseServices::{
        Tbsi_Context_Create, Tbsi_GetDeviceInfo, Tbsip_Context_Close, Tbsip_Submit_Command,
        TBS_COMMAND_LOCALITY_ZERO, TBS_COMMAND_PRIORITY_NORMAL, TBS_CONTEXT_PARAMS2,
        TBS_CONTEXT_PARAMS2_0, TBS_CONTEXT_PARAMS2_0_0, TPM_DEVICE_INFO,
    };

    const TBS_SUCCESS: u32 = 0;

    fn line(label: &str, result: &str) {
        println!("{label:<44} {result}");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// First 8 bytes only, so a pasted report does not carry a full machine fingerprint.
    fn short(bytes: &[u8]) -> String {
        let n = bytes.len().min(8);
        format!("{}... ({} bytes)", hex(&bytes[..n]), bytes.len())
    }

    fn is_elevated() -> Option<bool> {
        // SAFETY: querying our own process token with a correctly sized TOKEN_ELEVATION.
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
            let mut elev = TOKEN_ELEVATION::default();
            let mut ret = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                Some(&mut elev as *mut _ as *mut c_void),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret,
            );
            let _ = CloseHandle(token);
            ok.ok()?;
            Some(elev.TokenIsElevated != 0)
        }
    }

    fn secure_boot_enabled() -> Option<bool> {
        // SAFETY: RegGetValueW into a DWORD with the correct size.
        unsafe {
            let mut value = 0u32;
            let mut size = 4u32;
            let r = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State"),
                w!("UEFISecureBootEnabled"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut value as *mut _ as *mut c_void),
                Some(&mut size),
            );
            if r.0 == 0 {
                Some(value != 0)
            } else {
                None
            }
        }
    }

    struct Tbs(*mut c_void);

    impl Tbs {
        fn open() -> Result<Self, u32> {
            let params = TBS_CONTEXT_PARAMS2 {
                version: 2, // TBS_CONTEXT_VERSION_TWO
                Anonymous: TBS_CONTEXT_PARAMS2_0 {
                    Anonymous: TBS_CONTEXT_PARAMS2_0_0::default(),
                },
            };
            let mut p = params;
            // Flag bits: requestRaw = 0x1, includeTpm12 = 0x2, includeTpm20 = 0x4.
            p.Anonymous.asUINT32 = 0x4;
            let mut ctx: *mut c_void = std::ptr::null_mut();
            // SAFETY: p is a valid TBS_CONTEXT_PARAMS2; ctx receives the handle.
            let rc = unsafe { Tbsi_Context_Create(&p as *const _ as *const _, &mut ctx) };
            if rc == TBS_SUCCESS {
                Ok(Tbs(ctx))
            } else {
                Err(rc)
            }
        }

        fn submit(&self, cmd: &[u8]) -> Result<Vec<u8>, u32> {
            let mut out = vec![0u8; 4096];
            let mut out_len = out.len() as u32;
            // SAFETY: buffers are valid for the stated lengths; the context is open.
            let rc = unsafe {
                Tbsip_Submit_Command(
                    self.0,
                    TBS_COMMAND_LOCALITY_ZERO,
                    TBS_COMMAND_PRIORITY_NORMAL,
                    cmd,
                    out.as_mut_ptr(),
                    &mut out_len,
                )
            };
            if rc != TBS_SUCCESS {
                return Err(rc);
            }
            out.truncate(out_len as usize);
            Ok(out)
        }
    }

    impl Drop for Tbs {
        fn drop(&mut self) {
            // SAFETY: handle came from Tbsi_Context_Create and is closed once.
            unsafe {
                let _ = Tbsip_Context_Close(self.0);
            }
        }
    }

    fn be32(b: &[u8], off: usize) -> Option<u32> {
        b.get(off..off + 4)
            .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// TPM2_GetCapability(TPM_CAP_TPM_PROPERTIES, from TPM_PT_FIXED).
    fn get_properties(tbs: &Tbs) -> Result<Vec<(u32, u32)>, String> {
        let mut cmd = Vec::new();
        cmd.extend_from_slice(&[0x80, 0x01]); // TPM_ST_NO_SESSIONS
        cmd.extend_from_slice(&22u32.to_be_bytes());
        cmd.extend_from_slice(&0x0000_017Au32.to_be_bytes()); // TPM2_CC_GetCapability
        cmd.extend_from_slice(&6u32.to_be_bytes()); // TPM_CAP_TPM_PROPERTIES
        cmd.extend_from_slice(&0x0000_0100u32.to_be_bytes()); // TPM_PT_FIXED
        cmd.extend_from_slice(&0x20u32.to_be_bytes()); // count
        let resp = tbs
            .submit(&cmd)
            .map_err(|rc| format!("TBS error 0x{rc:08x}"))?;
        let rc = be32(&resp, 6).ok_or("short response")?;
        if rc != 0 {
            return Err(format!("TPM response code 0x{rc:08x}"));
        }
        // header(10) moreData(1) capability(4) count(4) then (property, value) pairs
        let count = be32(&resp, 15).ok_or("short response")? as usize;
        let mut props = Vec::new();
        for i in 0..count {
            let base = 19 + i * 8;
            match (be32(&resp, base), be32(&resp, base + 4)) {
                (Some(p), Some(v)) => props.push((p, v)),
                _ => break,
            }
        }
        Ok(props)
    }

    fn ascii4(v: u32) -> String {
        v.to_be_bytes()
            .iter()
            .map(|&c| {
                if (0x20..0x7f).contains(&c) {
                    c as char
                } else {
                    '?'
                }
            })
            .collect::<String>()
            .trim()
            .to_string()
    }

    /// TPM2_PCR_Read for one PCR in the SHA-256 bank.
    fn pcr_read(tbs: &Tbs, pcr: u8) -> Result<Vec<u8>, String> {
        let mut bitmap = [0u8; 3];
        bitmap[(pcr / 8) as usize] = 1 << (pcr % 8);
        let mut cmd = Vec::new();
        cmd.extend_from_slice(&[0x80, 0x01]);
        cmd.extend_from_slice(&20u32.to_be_bytes());
        cmd.extend_from_slice(&0x0000_017Eu32.to_be_bytes()); // TPM2_CC_PCR_Read
        cmd.extend_from_slice(&1u32.to_be_bytes()); // one selection
        cmd.extend_from_slice(&0x000Bu16.to_be_bytes()); // TPM_ALG_SHA256
        cmd.push(3);
        cmd.extend_from_slice(&bitmap);
        let resp = tbs
            .submit(&cmd)
            .map_err(|rc| format!("TBS error 0x{rc:08x}"))?;
        let rc = be32(&resp, 6).ok_or("short response")?;
        if rc != 0 {
            return Err(format!("TPM response code 0x{rc:08x}"));
        }
        // header(10) updateCounter(4) selOut: count(4) hash(2) size(1)+3 ; digests: count(4) size(2) digest
        let off = 10 + 4 + 4 + 2 + 1 + 3;
        let n = be32(&resp, off).ok_or("short response")?;
        if n != 1 {
            return Err("unexpected PCR count".into());
        }
        let dlen = resp
            .get(off + 4..off + 6)
            .map(|s| u16::from_be_bytes([s[0], s[1]]) as usize)
            .ok_or("short response")?;
        resp.get(off + 6..off + 6 + dlen)
            .map(|d| d.to_vec())
            .ok_or_else(|| "short response".to_string())
    }

    fn ncrypt_checks() {
        // SAFETY: standard NCrypt calls; every handle is freed on all paths.
        unsafe {
            let mut prov = NCRYPT_PROV_HANDLE::default();
            if let Err(e) = NCryptOpenStorageProvider(&mut prov, MS_PLATFORM_CRYPTO_PROVIDER, 0) {
                line(
                    "Platform Crypto Provider (TPM key store)",
                    &format!("UNAVAILABLE ({e})"),
                );
                return;
            }
            line("Platform Crypto Provider (TPM key store)", "opened");

            // Ephemeral (unnamed) TPM-resident ECDSA P-256 key: created and freed at once.
            let mut key = NCRYPT_KEY_HANDLE::default();
            let created = NCryptCreatePersistedKey(
                prov,
                &mut key,
                BCRYPT_ECDSA_P256_ALGORITHM,
                PCWSTR::null(),
                windows::Win32::Security::Cryptography::CERT_KEY_SPEC(0),
                NCRYPT_FLAGS(0),
            )
            .and_then(|_| NCryptFinalizeKey(key, NCRYPT_FLAGS(0)));
            match created {
                Ok(()) => line(
                    "Ephemeral TPM P-256 key as this user",
                    "CREATED (not persisted, freed immediately)",
                ),
                Err(e) => line(
                    "Ephemeral TPM P-256 key as this user",
                    &format!("FAILED ({e})"),
                ),
            }
            if key.0 != 0 {
                let _ =
                    NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(key.0));
            }

            for (name, label) in [
                (
                    w!("PCP_PLATFORM_TYPE"),
                    "Provider property PCP_PLATFORM_TYPE",
                ),
                (w!("PCP_EKPUB"), "Endorsement key public (PCP_EKPUB)"),
                (
                    w!("PCP_EKNVCERT"),
                    "EK certificate in TPM NV (PCP_EKNVCERT)",
                ),
            ] {
                let mut buf = vec![0u8; 4096];
                let mut got = 0u32;
                match NCryptGetProperty(
                    windows::Win32::Security::Cryptography::NCRYPT_HANDLE(prov.0),
                    name,
                    Some(&mut buf),
                    &mut got,
                    windows::Win32::Security::OBJECT_SECURITY_INFORMATION(0),
                ) {
                    Ok(()) => line(label, &format!("READABLE {}", short(&buf[..got as usize]))),
                    Err(e) => line(label, &format!("NOT READABLE ({e})")),
                }
            }
            attestation_checks(prov);
            let _ = NCryptFreeObject(windows::Win32::Security::Cryptography::NCRYPT_HANDLE(
                prov.0,
            ));
        }
    }

    /// Owns a key handle; a named key is deleted from the key store on drop, an ephemeral key
    /// is simply released. Either way nothing outlives the probe.
    struct KeyGuard {
        key: NCRYPT_KEY_HANDLE,
        delete_named: bool,
    }

    impl Drop for KeyGuard {
        fn drop(&mut self) {
            if self.key.0 == 0 {
                return;
            }
            // SAFETY: the handle was created by NCryptCreatePersistedKey and is released once.
            unsafe {
                if self.delete_named {
                    // NCryptDeleteKey also frees the handle on success.
                    if NCryptDeleteKey(self.key, 0).is_ok() {
                        return;
                    }
                }
                let _ = NCryptFreeObject(NCRYPT_HANDLE(self.key.0));
            }
        }
    }

    fn make_key(
        prov: NCRYPT_PROV_HANDLE,
        alg: PCWSTR,
        name: PCWSTR,
        identity: bool,
    ) -> windows::core::Result<KeyGuard> {
        // SAFETY: standard NCrypt key creation; the guard releases the handle on every path.
        unsafe {
            let mut key = NCRYPT_KEY_HANDLE::default();
            NCryptCreatePersistedKey(prov, &mut key, alg, name, CERT_KEY_SPEC(0), NCRYPT_FLAGS(0))?;
            let guard = KeyGuard {
                key,
                delete_named: !name.is_null(),
            };
            if identity {
                NCryptSetProperty(
                    NCRYPT_HANDLE(key.0),
                    NCRYPT_PCP_KEY_USAGE_POLICY_PROPERTY,
                    &NCRYPT_PCP_IDENTITY_KEY.to_le_bytes(),
                    NCRYPT_FLAGS(0),
                )?;
            }
            NCryptFinalizeKey(key, NCRYPT_FLAGS(0))?;
            Ok(guard)
        }
    }

    fn random_bytes<const N: usize>() -> [u8; N] {
        let mut b = [0u8; N];
        // SAFETY: system-preferred RNG, buffer valid for N bytes.
        unsafe {
            let _ = BCryptGenRandom(None, &mut b, BCRYPTGENRANDOM_FLAGS(2));
        }
        b
    }

    /// Key attestation and platform claim through the Platform Crypto Provider, as a normal user.
    /// The attestation key (AIK) is NOT activated against an EK certificate, so even a produced claim
    /// only shows "signed by some TPM-resident key"; the OS self-check below is not a remote proof.
    fn attestation_checks(prov: NCRYPT_PROV_HANDLE) {
        println!("-- attestation (claims; AIK not activated, nothing persisted) --");
        let aik = match make_key(prov, BCRYPT_RSA_ALGORITHM, PCWSTR::null(), true) {
            Ok(k) => {
                line("Ephemeral identity key (AIK) as this user", "CREATED");
                k
            }
            Err(e) => {
                line(
                    "Ephemeral identity key (AIK) as this user",
                    &format!("FAILED ({e})"),
                );
                // Some TPM stacks only allow named identity keys. Try one with a random name that
                // is deleted again by the guard.
                let name: Vec<u16> = format!("kagilink-probe-{}\0", hex(&random_bytes::<6>()))
                    .encode_utf16()
                    .collect();
                match make_key(prov, BCRYPT_RSA_ALGORITHM, PCWSTR(name.as_ptr()), true) {
                    Ok(k) => {
                        line("Named temporary AIK (deleted after test)", "CREATED");
                        k
                    }
                    Err(e2) => {
                        line(
                            "Named temporary AIK (deleted after test)",
                            &format!("FAILED ({e2})"),
                        );
                        return;
                    }
                }
            }
        };
        let subject = match make_key(prov, BCRYPT_ECDSA_P256_ALGORITHM, PCWSTR::null(), false) {
            Ok(k) => k,
            Err(e) => {
                line("Subject key for attestation", &format!("FAILED ({e})"));
                return;
            }
        };

        // Key attestation claim: the AIK vouches that `subject` lives in this TPM.
        let mut nonce = random_bytes::<32>();
        let mut buffers = [BCryptBuffer {
            cbBuffer: nonce.len() as u32,
            BufferType: NCRYPTBUFFER_CLAIM_KEYATTESTATION_NONCE,
            pvBuffer: nonce.as_mut_ptr().cast(),
        }];
        let desc = BCryptBufferDesc {
            ulVersion: 0,
            cBuffers: 1,
            pBuffers: buffers.as_mut_ptr(),
        };
        let mut blob = vec![0u8; 8192];
        let mut got = 0u32;
        // SAFETY: desc/buffers/nonce outlive the call; blob is valid for its stated length.
        let made = unsafe {
            NCryptCreateClaim(
                Some(subject.key),
                Some(aik.key),
                NCRYPT_CLAIM_AUTHORITY_AND_SUBJECT,
                Some(&desc),
                Some(&mut blob),
                &mut got,
                0,
            )
        };
        match made {
            Ok(()) => {
                blob.truncate(got as usize);
                line(
                    "Key attestation claim (AIK vouches for key)",
                    &format!("CREATED {}", short(&blob)),
                );
                let mut out = BCryptBufferDesc::default();
                // SAFETY: same buffers; the blob is the one just produced.
                let verified = unsafe {
                    NCryptVerifyClaim(
                        subject.key,
                        Some(aik.key),
                        NCRYPT_CLAIM_AUTHORITY_AND_SUBJECT,
                        Some(&desc),
                        &blob,
                        &mut out,
                        0,
                    )
                };
                match verified {
                    Ok(()) => line(
                        "OS self-check of that claim (same machine)",
                        "OK (not a remote verifier's proof)",
                    ),
                    Err(e) => line(
                        "OS self-check of that claim (same machine)",
                        &format!("FAILED ({e})"),
                    ),
                }
            }
            Err(e) => line(
                "Key attestation claim (AIK vouches for key)",
                &format!("FAILED ({e})"),
            ),
        }

        // Platform claim: TPM quote over selected PCRs plus the measured boot log. The accepted
        // parameter layout is not obvious from the headers, so a few documented-looking variants
        // are tried and every outcome is reported.
        let variants: [(&str, u32, usize, usize); 5] = [
            ("mask 4 bytes, nonce 32", NCRYPT_CLAIM_PLATFORM, 4, 32),
            ("mask 4 bytes, nonce 20", NCRYPT_CLAIM_PLATFORM, 4, 20),
            ("mask 4 bytes, no nonce", NCRYPT_CLAIM_PLATFORM, 4, 0),
            ("mask 3 bytes, nonce 32", NCRYPT_CLAIM_PLATFORM, 3, 32),
            (
                "authority-only flag, mask 4, nonce 32",
                NCRYPT_CLAIM_PLATFORM | 1,
                4,
                32,
            ),
        ];
        let mut platform_ok = false;
        for (label, claim_type, mask_len, nonce_len) in variants {
            let mut mask = [0x81u8, 0, 0, 0]; // PCR 0 and PCR 7, little-endian bit mask
            let mut quote_nonce = random_bytes::<32>();
            let mut pbuf = vec![BCryptBuffer {
                cbBuffer: mask_len as u32,
                BufferType: NCRYPTBUFFER_TPM_PLATFORM_CLAIM_PCR_MASK,
                pvBuffer: mask.as_mut_ptr().cast(),
            }];
            if nonce_len > 0 {
                pbuf.push(BCryptBuffer {
                    cbBuffer: nonce_len as u32,
                    BufferType: NCRYPTBUFFER_TPM_PLATFORM_CLAIM_NONCE,
                    pvBuffer: quote_nonce.as_mut_ptr().cast(),
                });
            }
            let pdesc = BCryptBufferDesc {
                ulVersion: 0,
                cBuffers: pbuf.len() as u32,
                pBuffers: pbuf.as_mut_ptr(),
            };
            let mut pblob = vec![0u8; 65536];
            let mut pgot = 0u32;
            // SAFETY: pdesc/pbuf/mask/quote_nonce outlive the call; pblob is valid for its length.
            let pmade = unsafe {
                NCryptCreateClaim(
                    None,
                    Some(aik.key),
                    claim_type,
                    Some(&pdesc),
                    Some(&mut pblob),
                    &mut pgot,
                    0,
                )
            };
            match pmade {
                Ok(()) => {
                    line(
                        &format!("Platform claim [{label}]"),
                        &format!("CREATED {}", short(&pblob[..pgot as usize])),
                    );
                    platform_ok = true;
                    break;
                }
                Err(e) => line(
                    &format!("Platform claim [{label}]"),
                    &format!("FAILED ({e})"),
                ),
            }
        }
        if !platform_ok {
            line(
                "Platform claim (quote + boot log)",
                "NOT PRODUCED with any tried layout",
            );
        }
        drop(subject);
        drop(aik);
    }

    pub fn run() {
        println!("KagiLink TPM probe (read-only, local, claims only; D022)");
        println!("{}", "-".repeat(78));

        match is_elevated() {
            Some(e) => line(
                "Running as administrator (elevated)",
                if e {
                    "YES (results may differ from a normal user)"
                } else {
                    "no (normal user)"
                },
            ),
            None => line("Running as administrator (elevated)", "unknown"),
        }
        match secure_boot_enabled() {
            Some(true) => line("Secure Boot (registry claim)", "enabled"),
            Some(false) => line("Secure Boot (registry claim)", "disabled"),
            None => line(
                "Secure Boot (registry claim)",
                "not reported (legacy BIOS or no access)",
            ),
        }

        // SAFETY: TPM_DEVICE_INFO is the size and layout the API documents.
        let info = unsafe {
            let mut info = TPM_DEVICE_INFO::default();
            let rc = Tbsi_GetDeviceInfo(
                std::mem::size_of::<TPM_DEVICE_INFO>() as u32,
                &mut info as *mut _ as *mut c_void,
            );
            (rc == TBS_SUCCESS).then_some(info)
        };
        match info {
            Some(i) => {
                let ver = match i.tpmVersion {
                    1 => "TPM 1.2",
                    2 => "TPM 2.0",
                    _ => "unknown",
                };
                line("TPM present via TBS broker", &format!("YES, {ver}"));
                line(
                    "TPM interface type (TBS claim)",
                    &i.tpmInterfaceType.to_string(),
                );
            }
            None => {
                line(
                    "TPM present via TBS broker",
                    "NO (no TPM, disabled, or TBS blocked)",
                );
                return;
            }
        }

        let tbs = match Tbs::open() {
            Ok(t) => {
                line("TBS context as this user", "opened");
                t
            }
            Err(rc) => {
                line("TBS context as this user", &format!("FAILED 0x{rc:08x}"));
                return;
            }
        };
        match get_properties(&tbs) {
            Ok(props) => {
                line("TPM2_GetCapability as this user", "allowed");
                for (p, v) in props {
                    match p {
                        0x100 => line("  family indicator", &ascii4(v)),
                        0x105 => line("  manufacturer", &ascii4(v)),
                        0x10B => line("  firmware version (high)", &format!("0x{v:08x}")),
                        _ => {}
                    }
                }
            }
            Err(e) => line(
                "TPM2_GetCapability as this user",
                &format!("BLOCKED/FAILED ({e})"),
            ),
        }
        for pcr in [0u8, 7] {
            let label = format!("PCR {pcr} (SHA-256) read as this user");
            match pcr_read(&tbs, pcr) {
                Ok(d) => line(&label, &format!("allowed {}", short(&d))),
                Err(e) => line(&label, &format!("BLOCKED/FAILED ({e})")),
            }
        }
        drop(tbs);

        ncrypt_checks();

        println!("{}", "-".repeat(78));
        println!("These are claims from the OS/TPM on this machine. They do not show that the");
        println!("PC is free of malware, that its OS is trustworthy, or that a remote verifier");
        println!("would accept them. Nothing was written, persisted or sent anywhere.");
    }
}
