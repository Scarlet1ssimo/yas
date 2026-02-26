use std::panic;

use clap::{command, Command};
use yas::utils::press_any_key_to_continue;
use yas_genshin::application::ArtifactScannerApplication;
use yas_starrail::application::RelicScannerApplication;

const ERROR_LOG: &str = "yas_error.log";

fn get_genshin_command() -> Command {
    let cmd = ArtifactScannerApplication::build_command();
    cmd.name("genshin")
}

fn get_starrail_command() -> Command {
    let cmd = RelicScannerApplication::build_command();
    cmd.name("starrail")
}

fn init() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();
}

/// Write error/panic to yas_error.log so you can read it after the window closes (e.g. when UAC spawns another window).
fn write_error_log(s: &str) {
    let _ = std::fs::write(ERROR_LOG, s);
}

fn install_panic_log() {
    panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.as_str(),
                None => "Box<dyn Any>",
            },
        };
        let loc = info.location().map(|l| l.to_string()).unwrap_or_default();
        let _ = std::fs::write(
            ERROR_LOG,
            format!("panic: {}\n{}\n\nSee yas_error.log after the window closes.", msg, loc),
        );
    }));
}

pub fn main() {
    install_panic_log();
    init();
    let cmd = command!()
        .subcommand(get_genshin_command())
        .subcommand(get_starrail_command());
    let arg_matches = cmd.get_matches();

    let res = if let Some((subcommand_name, matches)) = arg_matches.subcommand() {
        if subcommand_name == "genshin" {
            let application = ArtifactScannerApplication::new(matches.clone());
            application.run()
        } else if subcommand_name == "starrail" {
            let application = RelicScannerApplication::new(matches.clone());
            application.run()
        } else {
            Ok(())
        }
    } else {
        Ok(())
    };

    match res {
        Ok(_) => {
            press_any_key_to_continue();
        },
        Err(e) => {
            let msg = format!("{}\n\nCaused by:\n{:?}", e, e);
            log::error!("error: {}", e);
            write_error_log(&msg);
            eprintln!("错误已写入 {}，请查看该文件", ERROR_LOG);
            press_any_key_to_continue();
        }
    }
}