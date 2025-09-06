use sea_orm::{
    ActiveModelBehavior, ColumnTrait, DatabaseConnection, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter,
};
use time::PrimitiveDateTime;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "FarmConfigInfo", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    pub local_volume: Option<i32>,
    pub speeker_state: Option<i32>,
    pub sound_column_pause: Option<i32>,
    pub sound_column_start_time: Option<PrimitiveDateTime>,
    pub alarm_content_lang: Option<String>,
    pub is_empty: bool,
    pub is_deleted: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

async fn find_one(db: &DatabaseConnection) -> anyhow::Result<Option<Model>> {
    let result = Entity::find()
        .filter(Column::IsDeleted.eq(false))
        .one(db)
        .await?;

    Ok(result)
}
