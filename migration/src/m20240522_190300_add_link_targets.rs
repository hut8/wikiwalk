use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LinkTargets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LinkTargets::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(LinkTargets::Title).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LinkTargets::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum LinkTargets {
    Table,
    Id,
    Title,
}
