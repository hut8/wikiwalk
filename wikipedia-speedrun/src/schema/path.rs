//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.6

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "path")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub source_page_id: i32,
    pub target_page_id: i32,
    pub timestamp: String,
    pub duration: f64,
    pub path_data: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
