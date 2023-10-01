use static_files::NpmBuild;

fn main() {
    NpmBuild::new("../wikiwalk-ui")
        .install().unwrap() // runs npm install
        .run("build").unwrap() // runs npm run build
        .target("../wikiwalk-ui/dist")
        .change_detection()
        .to_resource_dir()
        .build()
        .unwrap();
}
