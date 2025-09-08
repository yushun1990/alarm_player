use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, DatabaseConnection, DeriveEntityModel, DerivePrimaryKey,
    DeriveRelation, EnumIter, PrimaryKeyTrait,
};
use time::PrimitiveDateTime;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "TestAlarmPlayRecord", rename_all = "PascalCase")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: uuid::Uuid,
    // 设定开始时间
    pub plan_time: PrimitiveDateTime,
    // 实际测试开始时间
    pub test_time: PrimitiveDateTime,
    // 1: 音柱音箱 2: 本地电话 3: 邮箱 4: 公众号
    pub test_type: i32,
    // 通知对象: 音柱音箱为空， 其他分别对应手机号、邮箱帐号、公众号等
    pub notify_obj: Option<String>,
    // 录音文件，仅音柱音箱有
    pub media_file: Option<String>,
    // 1: 正常，2: 异常, 3: 未确认 4: 报警中断 5: 程序中断
    pub test_result: i32,
    // 测试过程中是否出现异常
    pub has_error: bool,
    // 异常信息
    pub err_message: Option<String>,
    // 记录创建时间
    pub creation_time: PrimitiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn insert(record: Model, db: &DatabaseConnection) -> anyhow::Result<()> {
    let record: ActiveModel = record.into();
    record.insert(db).await?;
    Ok(())
}
