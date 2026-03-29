use crate::{expect::SBSExpect, unwrap::SBSUnwrap};
use rhai::{Array, Engine};
use std::process::Command;

pub fn reg_engine_cmd(engine: &mut Engine) {
    engine.register_fn("cmd", |prc: String, args: Array| {
        let args_str = args
            .into_iter()
            .map(|arg| arg.try_cast::<String>().log_unwrap("Non-string argument"))
            .collect::<Vec<String>>();

        let output = Command::new(prc)
            .args(args_str)
            .output()
            .log_expect("Failed to execute process");

        print!("{}", String::from_utf8_lossy(&output.stdout));
        print!("{}", String::from_utf8_lossy(&output.stderr));

        output.status.code().unwrap_or(0) as i64
    });
}
