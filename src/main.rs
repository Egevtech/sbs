pub mod expect;
pub mod macros;
pub mod unwrap;

use std::{
    fs,
    path::Path,
    process::{self, Output, exit},
};

use clap::{Parser, Subcommand};
use knuffel;

use expect::SBSExpect;
use unwrap::SBSUnwrap;

// Main
#[derive(Subcommand, Clone, Debug, PartialEq)]
enum Command {
    /// Just build the project
    Build,

    /// Build project, then run its default target
    Run,

    /// Remove build directory
    Clean,

    /// Build project, then install its targets
    Install,
}

#[derive(Parser, Debug)]
#[clap(version)]
struct Cmd {
    /// Command to execute
    #[command(subcommand)]
    command: Command,

    /// Path to project configuration file
    #[arg(short, long, default_value = "sbs.kdl")]
    config: String,
}

#[derive(knuffel::Decode, Debug)]
struct KProject {
    #[knuffel(child, unwrap(argument))]
    name: String,

    #[knuffel(child, unwrap(children))]
    targets: Vec<Target>,
}

#[derive(knuffel::Decode, Debug)]
struct Target {
    #[knuffel(node_name)]
    name: String,

    #[knuffel(child, unwrap(arguments))]
    compile_args: Option<Vec<String>>,

    #[knuffel(child, unwrap(arguments))]
    link_args: Option<Vec<String>>,

    #[knuffel(child, unwrap(arguments))]
    sources: Vec<String>,

    #[knuffel(child, unwrap(argument))]
    install_directory: Option<String>,

    #[knuffel(child, unwrap(argument))]
    compiler: Option<String>,

    #[knuffel(child, unwrap(argument))]
    linker: Option<String>,

    #[knuffel(child, unwrap(argument))]
    language: Option<String>,

    #[knuffel(child, unwrap(argument))]
    r#type: Option<String>,
}

fn main() {
    let args: Cmd = Cmd::parse();

    let mut project: KProject = knuffel::parse::<KProject>(
        args.config.clone().as_str(),
        std::fs::read_to_string(args.config.clone())
            .log_expect(format!("Failed to read project file {}", args.config).as_str())
            .as_str(),
    )
    .log_expect("Failed to parse config file");

    match args.command {
        Command::Clean => {
            fs::remove_dir_all("./build").log_expect("Failed to remove build directory");
            exit(0);
        }

        Command::Build => {
            build_project(&mut project);
        }

        Command::Run => {
            log!(OOPS, "Coming soon, sorry");
        }

        Command::Install => project.targets.iter().for_each(|target| {
            install_target(target, "./build".to_string());
        }),
    }
}

fn install_target(target: &Target, build_directory: String) {
    if !fs::exists(build_directory.as_str()).log_expect("Failed to get project directory state") {
        log!(PANIC, "Can't access project build directory");
    }

    if !fs::exists(target.install_directory.clone().unwrap())
        .log_expect("Failed to get install directory state")
    {
        log!(PANIC, "Can't access install directory");
    }

    fs::copy(
        build_directory.as_str(),
        format!(
            "{}/{}",
            target.install_directory.clone().unwrap(),
            target.name
        ),
    )
    .log_expect("Failed to copy target");
}

fn build_project(project: &mut KProject) -> () {
    println!("Building project '{}'", project.name);

    project.targets.iter_mut().for_each(|target| {
        target.language = Some(
            target
                .language
                .as_ref()
                .unwrap_or(&String::from("c"))
                .to_string(),
        );

        if target.language != Some(String::from("c"))
            && target.language != Some(String::from("cpp"))
        {
            log!(
                PANIC,
                "SBS only supports C(c) and C++(cpp), not '{}' at target {}",
                target.language.as_ref().unwrap(),
                target.name
            );
        }

        if target.r#type.is_none() {
            target.r#type = Some(String::from("binary"))
        } else if target.r#type != Some(String::from("binary"))
            && target.r#type != Some(String::from("static"))
        {
            log!(
                PANIC,
                "SBS only supports binary and static output type, not '{}' at target {}",
                target.r#type.as_ref().unwrap(),
                target.name
            );
        }
    });

    project
        .targets
        .iter()
        .enumerate()
        .for_each(|(index, target)| {
            let output_file = if target.r#type == Some("binary".to_string()) {
                target.name.clone()
            } else {
                format!("lib{}.a", target.name)
            };

            build_target(target, output_file.clone());

            println!(
                "[{}%] Built target {} ({}/{})",
                ((index as f32 + 1f32) / project.targets.len() as f32 * 100f32) as i32,
                target.name,
                index + 1,
                project.targets.len(),
            );
        });
}

fn build_target(target: &Target, output_file: String) -> () {
    let files = target
        .sources
        .iter()
        .map(|f| {
            Path::new(f)
                .file_name()
                .log_unwrap("Can't get filename")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<String>>();

    target
        .sources
        .iter()
        .enumerate()
        .for_each(|(index, source)| {
            let name_only = Path::new(source)
                .file_name()
                .log_unwrap("Path resolution error")
                .to_string_lossy()
                .to_owned();

            println!(
                "[{}%] Building object {}",
                ((index + 1) as f32 / target.sources.len() as f32 * 100f32) as i32,
                name_only
            );

            std::fs::create_dir_all(format!("build/{}-target", target.name))
                .log_expect("Failed to create output directory");

            let output =
                process::Command::new(target.compiler.clone().unwrap_or("clang".to_string()))
                    .args([
                        "-c",
                        source.as_str(),
                        "-o",
                        format!("build/{}-target/{}.o", target.name, name_only).as_str(),
                    ])
                    .args(target.compile_args.clone().unwrap_or(vec![]))
                    .output()
                    .log_expect("Failed to execute compiler");

            print!(
                "{}",
                String::from_utf8(output.stdout.clone())
                    .log_expect("Uncorrected UTF-8 output format"),
            );

            print!(
                "{}",
                String::from_utf8(output.stderr.clone())
                    .log_expect("Uncorrected UTF-8 output format"),
            );

            if !output.status.success() {
                log!(PANIC, "Compiler panicked");
            }
        });

    println!("[LINK] Linking target {}...", target.name);

    let object_files = files
        .iter()
        .map(|filename| format!("build/{}-target/{}.o", target.name, filename))
        .collect::<Vec<String>>();

    let output: Output;
    if target.r#type == Some(String::from("static")) {
        output = process::Command::new("ar")
            .args(["rcs", format!("build/{output_file}").as_str()])
            .args(object_files)
            .args(target.link_args.clone().unwrap_or(vec![]))
            .output()
            .log_expect("Failed to execute linker");
    } else {
        output = process::Command::new(target.linker.clone().unwrap_or("clang".to_string()))
            .args(object_files)
            .args(["-o", format!("build/{output_file}").as_str()])
            .args(target.link_args.clone().unwrap_or(vec![]))
            .output()
            .log_expect("Failed to execute linker")
    }

    print!(
        "{}",
        String::from_utf8(output.stdout.clone()).log_expect("Uncorrected UTF-8 output format")
    );
    print!(
        "{}",
        String::from_utf8(output.stderr.clone()).log_expect("Uncorrected UTF-8 output format")
    );

    if !output.status.success() {
        eprintln!(
            "Linker panicked with status {}",
            output
                .status
                .code()
                .log_unwrap("Failed to get linker exit code")
        );
        exit(-1);
    }
}
