// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::ExitCode;

fn main() -> ExitCode {
    match rile::app::run(std::env::args_os()) {
        Ok(mode) => match mode {
            rile::app::RunMode::Help | rile::app::RunMode::Version => {
                if let Some(message) = mode.message() {
                    println!("{message}");
                }
                ExitCode::SUCCESS
            }
            rile::app::RunMode::Edit(options) => {
                match rile::terminal::run_basic_editor(options.file.as_deref()) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("rile: {error}");
                        ExitCode::FAILURE
                    }
                }
            }
        },
        Err(error) => {
            eprintln!("rile: {error}");
            ExitCode::FAILURE
        }
    }
}
