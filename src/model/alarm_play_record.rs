use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, DatabaseConnection, DeriveEntityModel, DeriveRelation,
    EnumIter,
};
use time::PrimitiveDateTime;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "AlarmRecordStorage", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    pub house_code: String,
    pub house_name: String,
    // `音柱报警/音箱报警`
    pub receiver_name: String,
    // 录音文件
    pub receiver_sign: String,
    pub alarm_time: PrimitiveDateTime,
    // 固定 `场舍端警报`
    pub alarm_grade: String,
    // `!has_error`
    pub sending_state: bool,
    // Box/Sound
    pub alarm_send_to: String,
    pub source_message: String,
    // 音柱/音箱未启用
    pub error_message: String,
    pub creation_time: PrimitiveDateTime,
    pub is_deleted: bool,
    // 固定 0
    pub alarm_client: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

async fn insert(record: Model, db: &DatabaseConnection) -> anyhow::Result<()> {
    let record: ActiveModel = record.into();
    record.insert(db).await?;
    Ok(())
}
