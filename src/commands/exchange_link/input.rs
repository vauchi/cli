// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{BufRead, IsTerminal, Read};

use anyhow::{Result, bail};
use vauchi_core::exchange::link_pake::LinkCode;
use zeroize::Zeroizing;

pub(super) fn confirmation_code(from_stdin: bool) -> Result<Zeroizing<String>> {
    let stdin = std::io::stdin();
    let typed = if from_stdin {
        let mut typed = Zeroizing::new(String::new());
        // Six digits, one separator and CRLF fit below ten bytes; cap before allocation.
        let read = stdin.lock().take(10).read_line(&mut typed);
        if read.is_err() || typed.len() >= 10 {
            bail!("expected a 6-digit code");
        }
        typed
    } else if stdin.is_terminal() {
        Zeroizing::new(
            dialoguer::Password::new()
                .with_prompt("6-digit code (sent separately)")
                .interact()?,
        )
    } else {
        bail!("read the separately delivered code with --code-stdin");
    };
    LinkCode::parse(&typed).map_err(|_| anyhow::anyhow!("expected a 6-digit code"))?;
    Ok(typed)
}
