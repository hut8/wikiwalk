use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Search::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Search::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Search::SourcePageId).integer().not_null())
                    .col(ColumnDef::new(Search::TargetPageId).integer().not_null())
                    .col(ColumnDef::new(Search::Timestamp).timestamp().not_null())
                    .col(ColumnDef::new(Search::Duration).float().not_null())
                    .col(ColumnDef::new(Search::PathId).integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        manager
            .drop_table(Table::drop().table(Search::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Search {
    Table,
    Id,
    SourcePageId,
    TargetPageId,
    Timestamp,
    Duration,
    PathId
}
