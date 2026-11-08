mod android;
mod json;
mod toml_simple;
mod archive;
mod png_decode;
mod interrupt;
mod cli;
mod config;
mod desktop;
mod dev;
mod monitor;
mod icons;
mod icon_layout;
mod installer_tools;
mod wix_paths;
mod project;
mod tools;

use std::error::Error;

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
pub fn bail<T>(message: impl Into<String>) -> Result<T> { Err(message.into().into()) }

fn main() {
    if let Err(e) = run() {
        eprintln!("mata: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cmd = cli::parse(std::env::args().skip(1))?;
    match cmd {
        cli::Command::Help => print!("{}", cli::HELP),
        cli::Command::AndroidInit(mode, opts) => {
            let root = tools::project_root(opts.project_dir.as_deref())?;
            project::android_init(&root, mode)?;
        }
        cli::Command::Init(opts) => {
            let root = tools::project_root(opts.project_dir.as_deref())?;
            project::init(&root)?;
        }
        cli::Command::Doctor(opts) => tools::doctor(&opts)?,
        cli::Command::Icon(source, opts) => {
            let root = tools::project_root(opts.project_dir.as_deref())?;
            icons::generate(&root, &source)?;
        },
        cli::Command::Build(platform, opts) => {
            // A mixed --bundle list builds each target platform once, avoiding
            // multiple Cargo compiles for MSI+NSIS or DEB+RPM in the same call.
            let mut platforms = Vec::new();
            if opts.bundles.is_empty() { platforms.push(platform); }
            else {
                for bundle in &opts.bundles {
                    if !platforms.contains(&bundle.platform()) { platforms.push(bundle.platform()); }
                }
            }
            for target_platform in platforms {
                let mut one_os = opts.clone();
                one_os.bundles.retain(|bundle| bundle.platform() == target_platform);
                execute(target_platform, &one_os, false)?;
            }
        },
        cli::Command::Dev(platform, opts) => dev::run(platform, &opts)?,
        cli::Command::Monitor(platform, opts) => monitor::run(platform, &opts)?,
    }
    Ok(())
}

pub fn execute(platform: cli::Platform, opts: &cli::Options, dev: bool) -> Result<()> {
    let root = tools::project_root(opts.project_dir.as_deref())?;
    let config = config::load(&root)?;
    let project = project::Project::discover(&root, &config, opts)?;
    match platform {
        cli::Platform::Android => {
            let output = android::build(&project, &config, opts, dev)?;
            println!("APK: {}", output.display());
            if dev { android::install_and_launch(&output, &config, opts)?; }
        }
        cli::Platform::Linux | cli::Platform::Windows => {
            let binary = desktop::build(&project, &config, platform, opts)?;
            if !dev { desktop::package(&project, &config, platform, opts, &binary)?; }
        }
    }
    Ok(())
}
