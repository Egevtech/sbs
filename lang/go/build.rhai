fn build_project(project, config) {
    print(`Building ${project.name} ${project.version}`);

    let args = ["build", "."];

    let result = cmd("go", args);
    if !(result == 0) {
        return -1;
    }

    return 0;
}

fn clean(project) {
    rmdir("build")
}
