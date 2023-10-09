use static_files::NpmBuild;

fn main() {
    if std::env::var("WIKIWALK_SKIP_FRONTEND_BUILD").is_ok() {
        static_files::resource_dir("../wikiwalk-ui/dist")
            .build()
            .unwrap();
        return;
    }
    NpmBuild::new("../wikiwalk-ui")
        .install()
        .unwrap() // runs npm install
        .run("build")
        .unwrap() // runs npm run build
        .target("../wikiwalk-ui/dist")
        .change_detection()
        .to_resource_dir()
        .build()
        .unwrap();
}
