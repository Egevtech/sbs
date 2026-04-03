use std::path::PathBuf;

use crate::cmdrhai::reg_engine_cmd;
use crate::expect::SBSExpect;
use crate::log;
use crate::to_array::ToArray;
use crate::unwrap::SBSUnwrap;
use crate::{KProject, RBConfig};
use rhai::{Array, CustomType, Engine, Scope, TypeBuilder};

#[derive(Clone, CustomType)]
pub struct EProject {
    pub name: String,
    pub version: String,

    pub compile_args: Array,
    pub link_args: Array,

    pub lib_headers: Array,
    pub header_outputs: Array,

    pub sources: Array,

    pub additional_files: Array,
    pub outputs: Array,
}

impl EProject {
    fn from(project: KProject) -> EProject {
        EProject {
            name: project.name,
            version: project.version,

            compile_args: project.compile_args.to_array(),
            link_args: project.link_args.to_array(),

            sources: project.sources.clone().to_array(),

            lib_headers: project.lib_headers.clone().to_array(),
            header_outputs: project
                .lib_headers
                .clone()
                .iter_mut()
                .map(|x| {
                    String::from(
                        std::path::Path::new(x)
                            .file_name()
                            .log_unwrap("Path resolution error")
                            .to_string_lossy(),
                    )
                })
                .collect::<Vec<String>>()
                .to_array(),

            additional_files: project.additional_files.to_array(),
            outputs: project.outputs.to_array(),
        }
    }
}

pub fn build_project(project: KProject, config: RBConfig) {
    let mut engine = Engine::new();

    engine.build_type::<EProject>();
    engine.build_type::<RBConfig>();

    engine.register_fn("cp", |src: String, target: String| {
        std::fs::copy(src.clone(), target.clone())
            .log_expect(format!("Failed to copy object {} to {}", src, target).as_str());
    });

    reg_engine_cmd(&mut engine);

    std::fs::create_dir_all(format!("build/{}-target", project.name).as_str())
        .log_expect("Failed to create build directory");

    let ast = engine
        .compile_file(
            [
                "/opt",
                "sbs",
                "lang",
                project.language.as_str(),
                "build.rhai",
            ]
            .iter()
            .collect::<PathBuf>(),
        )
        .log_expect("Failed to compile language configuration");

    let result = engine
        .call_fn::<i64>(
            &mut Scope::new(),
            &ast,
            "build_project",
            (EProject::from(project), config),
        )
        .log_expect("Failed to exec build function");

    if result != 0 {
        log!(PANIC, "Build finished with non-0 exit code({result})");
    }
}

pub fn clean_project(project: KProject) {
    let mut engine: Engine = Engine::new();

    engine.register_fn("rmdir", |path: String| {
        println!("cleaning dir {path}"); // TODO: remove
        std::fs::remove_dir_all(path).log_expect("Failed to remove dir");
    });

    engine.register_fn("rm", |path: String| {
        println!("cleaning {path}"); // TODO: remove
        std::fs::remove_file(path).log_expect("Failed to remove path");
    });

    engine.build_type::<EProject>();

    let ast = engine
        .compile_file(
            [
                "/opt",
                "sbs",
                "lang",
                project.language.as_str(),
                "clean.rhai",
            ]
            .iter()
            .collect::<PathBuf>(),
        )
        .log_expect("Failed to compile language clean script");

    let _ = engine.call_fn::<()>(
        &mut Scope::new(),
        &ast,
        "clean",
        (EProject::from(project), ()),
    );
}
