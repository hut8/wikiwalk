use static_files::NpmBuild;
use std::path::PathBuf;

fn main() {
    // Get absolute path to wikiwalk-ui
    let ui_dir = PathBuf::from("../wikiwalk-ui")
        .canonicalize()
        .expect("wikiwalk-ui directory should exist");
    let ui_dist = ui_dir.join("dist");
    
    if std::env::var("WIKIWALK_SKIP_FRONTEND_BUILD").is_ok() {
        static_files::resource_dir(ui_dist)
            .build()
            .unwrap();
        return;
    }
    
    NpmBuild::new(ui_dir)
        .install()
        .unwrap() // runs npm install
        .run("build")
        .unwrap() // runs npm run build
        .target(ui_dist)
        .change_detection()
        .to_resource_dir()
        .build()
        .unwrap();
}
