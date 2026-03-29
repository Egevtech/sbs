pub mod cmdrhai;
pub mod expect;
pub mod langscript;
pub mod macros;
pub mod to_array;
pub mod unwrap;

use std::{fs, path::Path};

use clap::{Parser, Subcommand};

use expect::SBSExpect;

use glob::glob;

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Position, TypeBuilder};

use crate::langscript::build_project;

#[derive(Parser, Clone, Debug, PartialEq, CustomType)]
pub struct RBConfig {
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
pub struct KProject {
    name: String,
    version: String,
    language: String,

    compile_args: Vec<String>,
    link_args: Vec<String>,

    sources: Vec<String>,

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
        |name: String, version: String, language: String| KProject {
            name,
            version,

            sources: vec![],
            compile_args: vec![],
            link_args: vec![],

            language,

            ..KProject::default()
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

    let project = engine
        .eval_file::<KProject>(Path::new(args.config.as_str()).to_path_buf())
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
            build_project(project, config);
            println!("Build finished");
        }
    }
}
