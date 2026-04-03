fn build_project(project, config) {
    print(`Building ${project.name} ${project.version}`);

    let objects = [];

    for (source, index) in project.sources {
        let out_name = "build/" + project.name + "-target/" + project.outputs[index];
        objects += out_name;

        let args = ["-c", "-o", out_name, source];
        args += project.compile_args;

        let result = cmd("clang", args);

        if result != 0 {
            print("Compilation failed, aborting...");

            return -1;
        }
    }

    let args = ["rcs", "build/lib" + project.name + ".a"];
    args += objects;

    let result = cmd("ar", args);

    if result != 0 {
        print(`Ar failed, it was with args: ${args}`);
        return -1;
    }

    for (header, index) in project.lib_headers {
        cp(header, "build/" + project.header_outputs[index]);
    }

    return 0;
}
