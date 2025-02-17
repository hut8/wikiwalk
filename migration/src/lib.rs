pub use sea_orm_migration::prelude::*;

mod m20221201_000001_search;
mod m20221223_003023_path;
mod m20240522_190300_add_link_targets;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20221201_000001_search::Migration),
            Box::new(m20221223_003023_path::Migration),
            Box::new(m20240522_190300_add_link_targets::Migration),
        ]
    }
}
