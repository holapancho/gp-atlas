//! `gp-atlas` binary. The egui frontend arrives in M1 (SPEC §12); for now this
//! only reports the pinned CLI contract.

use gp_atlas_core::{MIN_CLI_VERSION, manifest::Manifest};

fn main() -> std::process::ExitCode {
    println!("gp-atlas {}", env!("CARGO_PKG_VERSION"));
    println!("Requires Salesforce CLI (sf) {MIN_CLI_VERSION} or newer.");
    match Manifest::embedded() {
        Ok(m) => {
            println!(
                "Command manifest: {} commands for {}.",
                m.commands.len(),
                m.cli_version
            );
            println!("The desktop UI is not implemented yet (milestone M1).");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("embedded manifest is invalid: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
