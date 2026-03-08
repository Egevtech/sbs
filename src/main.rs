use std::{
    fs,
    path::Path,
    process::{self, Output, exit},
};

use clap::{Parser, Subcommand};
use knuffel;

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
    language: Option<String>,

    #[knuffel(child, unwrap(argument))]
    r#type: Option<String>,
}

fn main() {
    let args: Cmd = Cmd::parse();

    match args.command {
        Command::Clean => {
            fs::remove_dir_all("./build").expect("Failed to remove build directory");
            exit(0);
        }

        _ => {}
    }

    let mut project: KProject = knuffel::parse::<KProject>(
        args.config.clone().as_str(),
        std::fs::read_to_string(args.config.clone())
            .expect(format!("Failed to read project file {}", args.config).as_str())
            .as_str(),
    )
    .expect("Failed to parse config file");

    println!("Building project {}", project.name);

    project.targets.iter_mut().for_each(|v| {
        v.language = Some(
            v.language
                .as_ref()
                .unwrap_or(&String::from("c"))
                .to_string(),
        );

        if v.language != Some(String::from("c")) && v.language != Some(String::from("cpp")) {
            eprintln!(
                "SBS only support C(c) and C++(cpp), not '{}' at target {}",
                v.language.as_ref().unwrap(),
                v.name
            );
            exit(-1);
        }

        if v.r#type.is_none() {
            v.r#type = Some(String::from("binary"))
        } else if v.r#type != Some(String::from("binary"))
            && v.r#type != Some(String::from("static"))
        {
            eprintln!(
                "SBS only support binary and static output type, not '{}' at target {}",
                v.r#type.as_ref().unwrap(),
                v.name
            );
            exit(-1);
        }
    });

    let mut targets: Vec<(String, String)> = vec![];

    for (index, target) in project.targets.iter().enumerate() {
        let output_file = if target.r#type == Some("binary".to_string()) {
            target.name.clone()
        } else {
            format!("lib{}.a", target.name)
        };

        println!(
            "[{}%] Building target {} ({}/{})",
            ((index as f32) / project.targets.len() as f32 * 100f32) as i32,
            target.name,
            index + 1,
            project.targets.len(),
        );

        build_target(target, output_file.clone());

        if args.command == Command::Install && !target.install_directory.is_none() {
            targets.push((
                std::fs::canonicalize(format!("build/{}", output_file))
                    .expect("Path resolution error")
                    .to_str()
                    .unwrap()
                    .to_string(),
                format!(
                    "{}/{}",
                    target.install_directory.clone().unwrap(),
                    output_file
                ),
            ));
        }
    }

    println!("[100%] Installing targets...");

    let command = targets
        .iter_mut()
        .map(|(target, dest)| format!("cp {} {}", target, dest))
        .collect::<Vec<_>>()
        .join(" && ");

    process::Command::new("pkexec")
        .args(["sh", "-c", command.as_str()])
        .status()
        .expect("Failed to install files");

    println!("Finished");
}

fn build_target(target: &Target, output_file: String) -> () {
    let files = target
        .sources
        .iter()
        .map(|f| {
            String::from(
                Path::new(f)
                    .file_name()
                    .unwrap_or_else(|| panic!("OCHKO"))
                    .to_str()
                    .unwrap()
                    .to_string(),
            )
        })
        .collect::<Vec<String>>();

    target.sources.iter().for_each(|source| {
        let name_only = Path::new(source)
            .file_name()
            .unwrap_or_else(|| {
                eprintln!("Error in path resolution");
                exit(-1);
            })
            .to_str()
            .unwrap()
            .to_string();

        std::fs::create_dir_all(format!("build/{}-target", target.name))
            .expect("Failed to create output directory");

        let output = process::Command::new("clang")
            .args([
                "-c",
                source.as_str(),
                "-o",
                format!("build/{}-target/{}.o", target.name, name_only).as_str(),
            ])
            .args(if target.compile_args.is_none() {
                vec![]
            } else {
                target.compile_args.as_ref().unwrap().to_vec()
            })
            .output()
            .expect("Failed to execute compiler");

        print!(
            "{}",
            String::from_utf8(output.stdout.clone()).expect("Uncorrect UTF-8 output format"),
        );

        print!(
            "{}",
            String::from_utf8(output.stderr.clone()).expect("Uncorrect UTF-8 output format"),
        );

        if !output.status.success() {
            eprintln!(
                "Compiler paniced with status {}",
                output.status.code().unwrap()
            );
            exit(-1);
        }
    });

    println!("Linking target {}...", target.name);

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
            .expect("Failed to execute linker");
    // } else if target.r#type == Some(String::from("binary")) {
    } else {
        output = process::Command::new("clang")
            .args(object_files)
            .args(["-o", format!("build/{output_file}").as_str()])
            .args(target.link_args.clone().unwrap_or(vec![]))
            .output()
            .expect("Failed to execute linker")
    }

    print!(
        "{}",
        String::from_utf8(output.stdout.clone()).expect("Uncorrect UTF-8 output format")
    );
    print!(
        "{}",
        String::from_utf8(output.stderr.clone()).expect("Uncorrect UTF-8 output format")
    );

    if !output.status.success() {
        eprintln!(
            "Linker paniced with status {}",
            output.status.code().unwrap()
        );
        exit(-1);
    }
}
