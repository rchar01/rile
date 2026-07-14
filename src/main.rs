// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
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
                let runtime_options = rile::terminal::RuntimeOptions {
                    file: options.file.as_deref(),
                    visual_test: options.visual_test,
                    test_size: options.test_size.map(|size| rile::terminal::TerminalSize {
                        rows: size.rows,
                        columns: size.columns,
                    }),
                };
                match rile::terminal::run_basic_editor(runtime_options) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        let error = rile::terminal::escape_terminal_controls(&error.to_string());
                        eprintln!("rile: {error}");
                        ExitCode::FAILURE
                    }
                }
            }
        },
        Err(error) => {
            let error = rile::terminal::escape_terminal_controls(&error.to_string());
            eprintln!("rile: {error}");
            ExitCode::FAILURE
        }
    }
}
