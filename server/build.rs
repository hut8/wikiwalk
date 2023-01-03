use static_files::NpmBuild;

fn main() {
    NpmBuild::new("../ui")
        .install().unwrap() // runs npm install
        .run("build").unwrap() // runs npm run build
        .target("../ui/dist")
        .to_resource_dir()
        .build().unwrap();
}
