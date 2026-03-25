//! `SeaORM` entity for the `installed_resources` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "installed_resources")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub resource_type: String,
    pub version: String,
    pub channel: String,
    pub installed_at: String,
    pub status: String,
    pub config_path: String,
    pub data_path: String,
    pub validation: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::service_instance::Entity")]
    ServiceInstance,
}

impl Related<super::service_instance::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ServiceInstance.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
