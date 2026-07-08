// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::display;

/// Prints funding links and fund allocation info to the terminal.
pub fn run(locale: &str) {
    println!("{}", display::t("cli.cmd.support.title", locale));
    println!();
    println!("{}", display::t("cli.cmd.support.body_line1", locale));
    println!("{}", display::t("cli.cmd.support.body_line2", locale));
    println!();
    println!("{}", display::t("cli.cmd.support.github", locale));
    println!("{}", display::t("cli.cmd.support.liberapay", locale));
    println!();
    println!("{}", display::t("cli.cmd.support.where_funds_go", locale));
    println!("{}", display::t("cli.cmd.support.fund_hardware", locale));
    println!(
        "{}",
        display::t("cli.cmd.support.fund_infrastructure", locale)
    );
    println!("{}", display::t("cli.cmd.support.fund_security", locale));
    println!("{}", display::t("cli.cmd.support.fund_development", locale));
}
