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
    // 音箱音量
    pub local_volume: Option<i32>,
    // 音箱启用状态
    pub speaker_state: Option<i32>,
    // 报警暂停
    pub sound_column_pause: Option<i32>,
    // 报警暂停恢复时间
    pub sound_column_start_time: Option<PrimitiveDateTime>,
    // 报警语言
    pub alarm_content_lang: Option<String>,
    pub is_deleted: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn find_one(db: &DatabaseConnection) -> anyhow::Result<Option<Model>> {
    let result = Entity::find()
        .filter(Column::IsDeleted.eq(false))
        .one(db)
        .await?;

    Ok(result)
}
