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
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
enum Command {
    /// Just build the project
    Build(RBConfig),

    /// Build project, then run its default target
    Run(RBConfig),

    /// Remove build directory
    Clean,

    /// Build project, then install its targets
    Install{
        targets: Option<Vec<String>>,
    },
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
    targets: Vec<Target>,
}

#[derive(Default, CustomType, Clone, Debug)]
struct Target {
    name: String,

    compile_args: Vec<String>,

    link_args: Vec<String>,

    sources: Vec<String>,

    install_directory: Option<String>,

    compiler: String,

    linker: String,

    language: Option<String>,

    r#type: Option<String>,
}

impl Target {
    fn add_source(&mut self, source: String) {
        self.sources.push(source);
    }

    fn add_sources(&mut self, sources: Vec<String>) {
        self.sources.extend(sources);
    }
}

impl KProject {
}

fn main() {
    let args: Cmd = Cmd::parse();

    log!(INFO, "Starting engine");
    let mut engine: Engine = Engine::new();
    log!(INFO, "Registering types and functions...");
    engine.build_type::<Target>();

    engine.register_fn("init_target", |name: String| {
        let mut target: Target = Target::default();

        target.name = name;

        target.sources = vec![];
        target.compile_args = vec![];
        target.link_args = vec![];

        target.compiler = "clang".to_string();
        target.linker = "clang".to_string();

        target
    });

    engine.register_fn("init_project", |name: String| {
        let mut project: KProject = KProject::default();

        project.name = name;

        project
    });

    engine.register_fn("add_target", |project: &mut KProject, target: Target| {
        project.targets.push(target);
    });

    engine.register_fn("add_source", Target::add_source);
    engine.register_fn(
        "add_sources",
        |target: &mut Target, sources: rhai::Array| {
            target.add_sources(
                sources
                    .into_iter()
                    .filter_map(|source| source.try_cast::<String>())
                    .collect(),
            );
        },
    );

    engine.register_fn("set_type", |target: &mut Target, r#type: String| {
        target.r#type = Some(r#type);
    });

    engine.register_fn("add_compile_args", |target: &mut Target, compile_args: Vec<Dynamic>| {
        target.compile_args.extend(compile_args.into_iter().filter_map(|arg| arg.try_cast::<String>()));
    });

    engine.register_fn("add_link_args", |target: &mut Target, link_args: Vec<Dynamic>| {
        target.link_args.extend(link_args.into_iter().filter_map(|arg| arg.try_cast::<String>()));
    });

    engine.register_fn("set_installation_path", |target: &mut Target, installation_path: String| {
        target.install_directory = Some(installation_path);
    });

    engine.register_fn("set_compiler", |target: &mut Target, compiler: String| {
        target.compiler = compiler;
    });

    engine.register_fn("set_linker", |target: &mut Target, linker: String| {
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
        Command::Build (config) => config.options
            .unwrap_or(vec![])
            .into_iter()
            .map(Dynamic::from)
            .collect(),
        Command::Run (config) => config.options
            .unwrap_or(vec![])
            .into_iter()
            .map(Dynamic::from)
            .collect(),
        _ => Array::new(),
    }); // Things like USE_INTERPRETER

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
            build_project(&mut project, config.no_output);
        }

        Command::Run(_config) => {
            log!(OOPS, "Coming soon, sorry");
        }

        Command::Install{targets} => {
            project.targets.iter().filter(|target| {
                targets.clone().unwrap_or(project.targets.iter().map(|t| t.name.clone()).collect()).contains(&target.name)
            }).for_each(|target| {
                log!(INFO, "Installing target {}", target.name);
                install_target(target, "./build".to_string());
            })
        },
    }
}

fn install_target(target: &Target, build_directory: String) {
    if target.install_directory.is_none() {
        return
    }

    if !fs::exists(build_directory.as_str()).log_expect("Failed to get project directory state") {
        log!(PANIC, "Can't access project build directory");
    }

    if !fs::exists(
        target
            .install_directory
            .clone()
            .log_unwrap("Installation directory did not set"),
    )
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

fn build_project(project: &mut KProject, no_output: bool) -> () {
    println!("Building project '{}'", project.name);
    log!(INFO, "Building project {}", project.name);

    project.targets.iter_mut().for_each(|target| {
        log!(INFO, "Target '{}'", target.name);
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

        log!(INFO, "Target language ok");

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

        log!(INFO, "Target type ok");
    });

    log!(INFO, "Targets ok");

    project
        .targets
        .iter()
        .enumerate()
        .for_each(|(index, target)| {
            println!("[{}%] Building target {}",
                     ((index as f32 / project.targets.len() as f32) * 100f32) as i32,
                     target.name);

            let output_file = if target.r#type == Some("binary".to_string()) {
                target.name.clone()
            } else {
                format!("lib{}.a", target.name)
            };

            log!(INFO, "Target's output file ready");

            build_target(target, output_file.clone(), no_output);

            println!(
                "[{}%] Built target {} ({}/{})",
                ((index as f32 + 1f32) / (project.targets.len()) as f32 * 100f32) as i32,
                target.name,
                index + 1,
                project.targets.len(),
            );
        });
}

fn build_target(target: &Target, output_file: String, no_output: bool) -> () {
    log!(INFO, "Building target '{}'", target.name);
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
                (index as f32 / (target.sources.len() as f32 - 1f32) * 100f32) as i32,
                name_only
            );

            std::fs::create_dir_all(format!("build/{}-target", target.name))
                .log_expect("Failed to create output directory");

            log!(INFO, "Building object {}", target.name);

            let output =
                process::Command::new(target.compiler.clone())
                    .args([
                        "-c",
                        source.as_str(),
                        "-o",
                        format!("build/{}-target/{}.o", target.name, name_only).as_str(),
                    ])
                    .args(target.compile_args.clone())
                    .output()
                    .log_expect(format!("Failed to execute compiler: {:?}", target.compiler).as_str());

            if !no_output {
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

    println!("[LINK] Linking target {}...", target.name);
    log!(INFO, "Linking target {}", target.name);

    let object_files = files
        .iter()
        .map(|filename| format!("build/{}-target/{}.o", target.name, filename))
        .collect::<Vec<String>>();

    let output: Output;
    if target.r#type == Some(String::from("static")) {
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

    if !no_output {
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
