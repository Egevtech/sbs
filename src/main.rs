pub mod cmdrhai;
pub mod expect;
pub mod langscript;
pub mod macros;
pub mod to_array;
pub mod unwrap;

use std::path::Path;

use clap::{Parser, Subcommand};

use expect::SBSExpect;
use unwrap::SBSUnwrap;

use glob::glob;

use rhai::{Array, CustomType, Dynamic, Engine, EvalAltResult, Position, TypeBuilder};

use crate::langscript::{build_project, clean_project};

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
    outputs: Vec<String>,

    lib_headers: Vec<String>,

    additional_files: Vec<String>,

    local_dependencies: Vec<String>,

    r#type: String,
}

impl KProject {
    fn add_source(&mut self, source: String) {
        self.sources.push(source);
    }

    fn add_sources(&mut self, sources: Vec<String>) {
        self.sources.extend(sources);
    }

    fn prepare_outputs(&mut self) {
        self.outputs = self
            .sources
            .clone()
            .iter_mut()
            .map(|x| {
                String::from(
                    std::path::Path::new(x)
                        .file_name()
                        .log_unwrap("Path resolution error")
                        .to_string_lossy(),
                ) + ".o"
            })
            .collect::<Vec<String>>();
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

    engine.register_fn(
        "add_local_dependency",
        |target: &mut KProject, path: String| {
            target.local_dependencies.push(path);
        },
    );

    engine.register_fn("add_lib_header", |target: &mut KProject, header: String| {
        target.lib_headers.push(header);
    });

    engine.register_fn(
        "add_lib_headers",
        |target: &mut KProject, headers: rhai::Array| {
            headers
                .clone()
                .into_iter()
                .filter_map(|header| header.try_cast::<String>())
                .for_each(|header| target.lib_headers.push(header));
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

    let mut dependencies: Vec<KProject> = Vec::new();

    let mut project = engine
        .eval_file::<KProject>(Path::new(args.config.as_str()).to_path_buf())
        .log_expect("Failed to run project file");

    project.prepare_outputs();

    log!(INFO, "Engine done");

    log!(INFO, "Resolving dependencies");

    for dependency in project.clone().local_dependencies {
        log!(INFO, "Local dependency {}", dependency);

        let mut dependecy_project: KProject = engine
            .eval_file::<KProject>(
                Path::new(format!("{}/sbs.rhai", dependency).as_str()).to_path_buf(),
            )
            .log_expect("Failed to run dependency project file");

        dependecy_project.sources = dependecy_project
            .sources
            .iter_mut()
            .map(|x| format!("{}/{}", dependency, x))
            .collect::<Vec<String>>();

        dependecy_project.lib_headers = dependecy_project
            .lib_headers
            .iter_mut()
            .map(|x| format!("{}/{}", dependency, x))
            .collect::<Vec<String>>();

        dependecy_project.prepare_outputs();

        dependecy_project.outputs.iter().for_each(|output| {
            project.additional_files.push(format!(
                "build/{}-target/{}",
                dependecy_project.name, output
            ));
        });

        dependencies.push(dependecy_project);
    }

    #[cfg(debug_assertions)]
    println!("{:#?}", project);

    log!(INFO, "Matching commands...");
    match args.command {
        Command::Clean => {
            clean_project(project);
        }

        Command::Build(config) => {
            for dep in dependencies {
                build_project(dep, config.clone());
            }

            build_project(project, config);
            println!("Build finished");
        }
    }
}
