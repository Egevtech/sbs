fn build_project(project, config) {
    print(`Building ${project.name} ${project.version}...`);

    let compiler = "clang++";

    let objects = [];

    for (source,index) in project.sources {
        let out_name = "build/" + project.name + "-target/" + project.outputs[index];
        objects += out_name;

        let args = ["-c", "-o", out_name, source];
        args += project.compile_args;


        let result = cmd(compiler, args);

        if result != 0{
            print("Compilation failed, aborting...");
            return -1;
        }
    }


    let args = ["-o", "build/" + project.name];

    args += objects;
    args += project.link_args;

    let result = cmd(compiler, args);

    if !(result == 0) {
        return -1;
    }

    return 0;
}
