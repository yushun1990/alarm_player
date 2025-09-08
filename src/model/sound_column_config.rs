use sea_orm::{
    ActiveModelBehavior, ColumnTrait, DatabaseConnection, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "SoundColumnConfig", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    pub device_id: i32,
    pub speed: i32,
    pub enabled: bool,
    pub is_deleted: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn find_all(db: &DatabaseConnection) -> anyhow::Result<Vec<Model>> {
    let result = Entity::find()
        .filter(Column::IsDeleted.eq(false))
        .filter(Column::Enabled.eq(true))
        .all(db)
        .await?;

    Ok(result)
}
