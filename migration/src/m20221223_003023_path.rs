use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Path::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Path::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Path::SourcePageId).integer().not_null())
                    .col(ColumnDef::new(Path::TargetPageId).integer().not_null())
                    .col(ColumnDef::new(Path::Timestamp).timestamp().not_null())
                    .col(ColumnDef::new(Path::Duration).float().not_null())
                    .col(ColumnDef::new(Path::Data).json().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Path::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Path {
    Table,
    Id,
    SourcePageId,
    TargetPageId,
    Timestamp,
    Duration,
    Data,
}
