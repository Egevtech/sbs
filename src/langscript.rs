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

    pub sources: Array,
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
            outputs: project
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
                .collect::<Vec<String>>()
                .to_array(),
        }
    }
}

pub fn build_project(project: KProject, config: RBConfig) {
    let mut engine = Engine::new();

    engine.build_type::<EProject>();
    engine.build_type::<RBConfig>();

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
