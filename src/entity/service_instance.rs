//! `SeaORM` entity for the `service_instances` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "service_instances")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub resource_id: String,
    pub instance_name: String,
    /// JSON: `Vec<String>` (role ids)
    pub roles_provided: String,
    /// JSON: `Vec<String>` (role ids)
    pub roles_required: String,
    /// JSON: `Vec<ConfiguredVar>`
    pub variables: String,
    pub network: String,
    pub status: String,
    pub port: Option<i32>,
    /// JSON: `Vec<String>` (S3 paths)
    pub s3_paths: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::installed_resource::Entity",
        from = "Column::ResourceId",
        to = "super::installed_resource::Column::Id"
    )]
    InstalledResource,
}

impl Related<super::installed_resource::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InstalledResource.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
