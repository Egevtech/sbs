let bfc = target_init("hello_world");
let options = get_build_options();

for option in options {
    print(option)
}

bfc.add_source("main.c");
bfc.add_sources(["aboba", "zhopa"]);

[bfc]
