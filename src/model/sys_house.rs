use sea_orm::{
    ActiveModelBehavior, ColumnTrait, DatabaseConnection, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "SysHouse", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    pub name: String,
    pub enabled: bool,
    pub house_code: String,
    pub is_empty: bool,
    pub is_deleted: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn find_all(db: &DatabaseConnection) -> anyhow::Result<Vec<Model>> {
    let result = Entity::find()
        .filter(Column::Enabled.eq(true))
        .filter(Column::IsDeleted.eq(false))
        .all(db)
        .await?;
    Ok(result)
}
