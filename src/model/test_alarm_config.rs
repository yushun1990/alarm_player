use sea_orm::{
    ActiveModelBehavior, ColumnTrait, DatabaseConnection, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EntityTrait, EnumIter, PrimaryKeyTrait, QueryFilter,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "TestAlarmConfig", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    /// 测试时长，单位 s
    pub duration: Option<i32>,
    pub crontab: Option<String>,
    pub enabled: bool,
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
