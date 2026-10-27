//! nikpak - command-line archiver for the .nik format.

use nikpak::{archive, cli, envpath, error, license, recomp};
#[cfg(feature = "gui")]
use nikpak::gui;
use cli::{Command, Parsed};
use std::process::ExitCode;

fn main() -> ExitCode {
    recomp::install_panic_hook();
    // An installed nikpak keeps itself on the PATH, so `nikpak` works in any
    // command prompt. A build tree is ignored (see envpath).
    envpath::ensure_in_path();
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        // Started without arguments (e.g. double-clicked): open the window.
        #[cfg(feature = "gui")]
        match gui::run() {
            Ok(()) => return ExitCode::SUCCESS,
            Err(_) => eprintln!("(No graphical display available - showing the command-line usage.)"),
        }
        println!("nikpak {} - fast archiver for the .nik format", cli::VERSION);
        println!();
        println!("Usage: nikpak <command> [options] <archive.nik> [files...]");
        println!("Commands: a (add)  x (extract)  e (extract flat)  l (list)  t (test)  d (delete)  p (print)");
        println!();
        println!("Use 'nikpak -help' for all options and examples.");
        return ExitCode::from(7);
    }

    // ---- right-click menu of the file manager ----------------------------------
    match args[0].to_ascii_lowercase().as_str() {
        "--add" | "--add-here" => {
            #[cfg(feature = "gui")]
            {
                let mode = if args[0].eq_ignore_ascii_case("--add-here") { gui::ShellAdd::Here } else { gui::ShellAdd::Dialog };
                return match gui::run_shell(mode, nikpak::shell::absolute(&args[1..])) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("ERROR: {e}");
                        ExitCode::from(2)
                    }
                };
            }
            #[cfg(not(feature = "gui"))]
            {
                eprintln!("ERROR: this nikpak was built without its window (use: nikpak a archive.nik files...)");
                return ExitCode::from(7);
            }
        }
        "--install-menu" | "--uninstall-menu" => {
            #[cfg(feature = "gui")]
            {
                let all = args.iter().any(|a| a.eq_ignore_ascii_case("--all-users"));
                let r = if args[0].eq_ignore_ascii_case("--install-menu") { nikpak::shell::install(all) } else { nikpak::shell::uninstall(all) };
                return match r {
                    Ok(msg) => {
                        println!("{msg}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("ERROR: {e}");
                        ExitCode::from(2)
                    }
                };
            }
            #[cfg(not(feature = "gui"))]
            {
                eprintln!("ERROR: the right-click menu needs the full nikpak (built with its window).");
                return ExitCode::from(7);
            }
        }
        _ => {}
    }

    // ---- licence commands ----------------------------------------------------
    match args[0].to_ascii_lowercase().as_str() {
        "register" => {
            if args.len() < 4 {
                eprintln!("Usage: nikpak register \"Your Name\" your@email.com NPK1-XXXXX-...");
                return ExitCode::from(7);
            }
            let key = args[3..].join("");
            return match license::register(&args[1], &args[2], &key) {
                Ok(l) => {
                    println!("Thank you! NikPak is now registered to {}.", l.describe());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    ExitCode::from(7)
                }
            };
        }
        "license" | "licence" => {
            println!("{}", licence_status());
            return ExitCode::SUCCESS;
        }
        "unregister" => {
            println!("{}", if license::unregister() { "The licence was removed from this user account." } else { "No licence was stored for this user." });
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let parsed = match cli::parse(&args) {
        Ok(p) => p,
        Err(e) => return report(e),
    };

    let result = match parsed {
        Parsed::Help => {
            // a closed pipe (nikpak -help | more, then q) is not an error
            use std::io::Write;
            let mut out = std::io::stdout();
            let _ = write!(out, "{}", cli::HELP).and_then(|_| writeln!(out, "\n{}", licence_status()));
            return ExitCode::SUCCESS;
        }
        Parsed::Version => {
            use std::io::Write;
            let mut out = std::io::stdout();
            let _ = writeln!(out, "nikpak {}  (c) 2026 Nicolas Choukroun, distributed by Nikkoid LLC", cli::VERSION)
                .and_then(|_| writeln!(out, "{}", licence_status()));
            return ExitCode::SUCCESS;
        }
        Parsed::Run(o) => {
            // like RAR's console version: a one-line reminder in evaluation copies
            // (on stderr, so it never mixes with listings or piped output)
            if !o.quiet && license::current().is_none() {
                eprintln!(
                    "nikpak {} (c) Nicolas Choukroun / Nikkoid LLC - evaluation copy. Free for personal use; companies need a licence per seat: {}",
                    cli::VERSION,
                    license::BUY_URL
                );
            }
            match o.command {
            Command::Add => archive::create(&o),
            Command::ExtractFull | Command::ExtractFlat => archive::extract(&o),
            Command::List | Command::ListVerbose | Command::ListBare => archive::list(&o),
            Command::Test => archive::test(&o),
            Command::Delete => archive::delete(&o, None).map(|_| 0),
            Command::Print => archive::print(&o),
            Command::MediaTest => archive::media_test(&o),
            }
        }
    };

    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => report(e),
    }
}

fn licence_status() -> String {
    match license::current() {
        Some(l) => format!("Registered to {}.", l.describe()),
        None => format!(
            "Evaluation copy (shareware). Free for personal use; companies need one licence per seat.\n\
             Personal licence {}, company licence {}: {}\n\
             Register with:  nikpak register \"Your Name\" your@email.com <key>",
            license::PRICE_PERSONAL,
            license::PRICE_COMPANY,
            license::BUY_URL
        ),
    }
}

fn report(e: error::NikError) -> ExitCode {
    eprintln!("ERROR: {e}");
    ExitCode::from(e.exit_code())
}
