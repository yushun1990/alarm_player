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
    pub cron: Option<String>,
    pub sup_types: i32,
    pub enabled: bool,
    pub is_deleted: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn find_one(db: &DatabaseConnection) -> anyhow::Result<Option<Model>> {
    let result = Entity::find()
        .filter(Column::IsDeleted.eq(false))
        .filter(Column::Enabled.eq(true))
        .all(db)
        .await?;

    if result.is_empty() {
        return Ok(None);
    }

    for m in result {
        if m.sup_types & 0x01 == 1 {
            return Ok(Some(m));
        }
    }

    Ok(None)
}
