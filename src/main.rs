pub mod expect;
pub mod macros;
pub mod unwrap;

use std::{
    fs,
    path::Path,
    process::{self, Output},
};

use clap::{Parser, Subcommand};

use expect::SBSExpect;
use unwrap::SBSUnwrap;

use glob::glob;

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Position, TypeBuilder};

#[derive(Parser, Clone, Debug, PartialEq)]
struct RBConfig {
    /// Do not show compiler and linker output
    #[arg(long)]
    no_output: bool,

    /// Select compiler instead parameters in config
    #[arg(short, long)]
    compiler: Option<String>,

    /// Select compiler instead parameters in config
    #[arg(short, long)]
    linker: Option<String>,

    /// Build options
    options: Option<Vec<String>>,

    /// Show verbose output due operations
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
enum Command {
    /// Just build the project
    Build(RBConfig),

    /// Build project, then run its default target
    // Run(RBConfig),

    /// Remove build directory
    Clean,
}

#[derive(Parser, Debug, Clone)]
#[clap(version)]
struct Cmd {
    /// Command to execute
    #[command(subcommand)]
    command: Command,

    /// Path to project script file
    #[arg(short, long, default_value = "sbs.rhai")]
    config: String,
}

#[derive(Default, CustomType, Clone, Debug)]
struct KProject {
    name: String,
    version: String,
    language: String,

    compile_args: Vec<String>,
    link_args: Vec<String>,

    sources: Vec<String>,

    install_directory: Option<String>,

    compiler: String,
    linker: String,

    r#type: String,
}

impl KProject {
    fn add_source(&mut self, source: String) {
        self.sources.push(source);
    }

    fn add_sources(&mut self, sources: Vec<String>) {
        self.sources.extend(sources);
    }
}

fn main() {
    let args: Cmd = Cmd::parse();

    log!(INFO, "Starting engine");
    let mut engine: Engine = Engine::new();

    log!(INFO, "Registering types and functions...");
    engine.build_type::<KProject>();

    engine.register_fn(
        "init_project",
        |name: String, version: String, language: String| {
            let mut target: KProject = KProject::default();

            target.name = name;
            target.version = version;

            target.sources = vec![];
            target.compile_args = vec![];
            target.link_args = vec![];

            target.compiler = "clang".to_string();
            target.linker = "clang".to_string();

            target.language = language;

            target
        },
    );

    engine.register_fn(
        "init_project",
        |name: String, version: String, language: String, r#type: String| {
            let mut project = KProject::default();

            project.name = name;
            project.version = version;

            project.language = language;

            project.compiler = "clang".to_string();
            project.linker = "clang".to_string();

            project.r#type = r#type;

            project
        },
    );

    engine.register_fn("add_source", KProject::add_source);
    engine.register_fn(
        "add_sources",
        |target: &mut KProject, sources: rhai::Array| {
            target.add_sources(
                sources
                    .into_iter()
                    .filter_map(|source| source.try_cast::<String>())
                    .collect(),
            );
        },
    );

    engine.register_fn("set_type", |target: &mut KProject, r#type: String| {
        target.r#type = r#type;
    });

    engine.register_fn(
        "add_compile_args",
        |target: &mut KProject, compile_args: Vec<Dynamic>| {
            target.compile_args.extend(
                compile_args
                    .into_iter()
                    .filter_map(|arg| arg.try_cast::<String>()),
            );
        },
    );

    engine.register_fn(
        "add_link_args",
        |target: &mut KProject, link_args: Vec<Dynamic>| {
            target.link_args.extend(
                link_args
                    .into_iter()
                    .filter_map(|arg| arg.try_cast::<String>()),
            );
        },
    );

    engine.register_fn(
        "set_installation_path",
        |target: &mut KProject, installation_path: String| {
            target.install_directory = Some(installation_path);
        },
    );

    engine.register_fn("set_compiler", |target: &mut KProject, compiler: String| {
        target.compiler = compiler;
    });

    engine.register_fn("set_linker", |target: &mut KProject, linker: String| {
        target.linker = linker;
    });

    engine.register_fn("filter_dir", |pattern: String| {
        glob(pattern.as_str())
            .log_expect("Invalid pattern")
            .map(|gr| gr.log_expect("Invalid pattern (l2)").display().to_string())
            .map(Dynamic::from)
            .collect::<Vec<Dynamic>>()
    });

    let fn_args = args.clone();
    engine.register_fn("get_build_options", move || match fn_args.clone().command {
        Command::Build(config) => config
            .options
            .unwrap_or(vec![])
            .into_iter()
            .map(Dynamic::from)
            .collect(),
        _ => Array::new(),
    });

    log!(INFO, "Running engine");

    let mut project = engine
        .eval_file(Path::new(args.config.as_str()).to_path_buf())
        .log_expect("Failed to run project file");

    log!(INFO, "Engine done");

    #[cfg(debug_assertions)]
    println!("{:#?}", project);

    log!(INFO, "Matching commands...");
    match args.command {
        Command::Clean => {
            fs::remove_dir_all("./build").log_expect("Failed to remove build directory");
        }

        Command::Build(config) => {
            build_project(&mut project, config);
        }
    }
}

fn build_project(target: &KProject, config: RBConfig) -> () {
    let output_file = if target.r#type == "static".to_string() {
        format!("{}.a", target.name.to_lowercase())
    } else {
        target.name.clone().to_lowercase()
    };

    log!(INFO, "Building target '{}'", target.name);
    println!("Compiling {}...", target.name);
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

            std::fs::create_dir_all(format!("build/{}-target", target.name))
                .log_expect("Failed to create output directory");

            log!(INFO, "Building object {}", target.name);

            if config.verbose {
                println!("Precompiling object {}", name_only);
            }

            let output = process::Command::new(target.compiler.clone())
                .args([
                    "-c",
                    source.as_str(),
                    "-o",
                    format!("build/{}-target/{}.o", target.name, name_only).as_str(),
                ])
                .args(target.compile_args.clone())
                .output()
                .log_expect(format!("Failed to execute compiler: {:?}", target.compiler).as_str());

            if !config.no_output {
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
            }

            if !output.status.success() {
                log!(
                    PANIC,
                    "Compiler panicked with status {}",
                    output
                        .status
                        .code()
                        .log_unwrap("Failed to get compile exit code")
                );
            }
        });

    println!("Building {} {}", target.name, target.version);

    let object_files = files
        .iter()
        .map(|filename| format!("build/{}-target/{}.o", target.name, filename))
        .collect::<Vec<String>>();

    let output: Output;
    if target.r#type == String::from("static") {
        output = process::Command::new("ar")
            .args(["rcs", format!("build/{output_file}").as_str()])
            .args(object_files)
            .args(target.link_args.clone())
            .output()
            .log_expect("Failed to execute linker: ar rcs");
    } else {
        output = process::Command::new(target.linker.clone())
            .args(object_files)
            .args(["-o", format!("build/{output_file}").as_str()])
            .args(target.link_args.clone())
            .output()
            .log_expect(format!("Failed to execute linker: {:?}", target.linker).as_str());
    }

    if !config.no_output {
        print!(
            "{}",
            String::from_utf8(output.stdout.clone()).log_expect("Uncorrected UTF-8 output format")
        );
        print!(
            "{}",
            String::from_utf8(output.stderr.clone()).log_expect("Uncorrected UTF-8 output format")
        );
    }

    if !output.status.success() {
        log!(
            PANIC,
            "Linker panicked with status {}",
            output
                .status
                .code()
                .log_unwrap("Failed to get linker exit code")
        );
    }
}
