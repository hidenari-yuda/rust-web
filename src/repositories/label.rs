use super::RepositoryError;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use validator::Validate;

#[async_trait]
pub trait LabelRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, payload: CreateLabel) -> anyhow::Result<Label>;
    async fn find(&self, id: i32) -> anyhow::Result<Label>;
    async fn find_by_user(&self, id: i32) -> anyhow::Result<Vec<Label>>;
    async fn all(&self) -> anyhow::Result<Vec<Label>>;
    async fn update(&self, id: i32, payload: UpdateLabel) -> anyhow::Result<Label>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Label {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateLabel {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 100, message = "Over name length"))]
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct UpdateLabel {
    #[validate(length(min = 1, message = "Can not be empty"))]
    #[validate(length(max = 100, message = "Over name length"))]
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LabelRepositoryForDb {
    pool: PgPool,
}

impl LabelRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LabelRepository for LabelRepositoryForDb {
    async fn create(&self, payload: CreateLabel) -> anyhow::Result<Label> {
        let optional_label = sqlx::query_as::<_, Label>(
            r#"
            select id, name from labels where name = $1
            "#,
        )
        .bind(payload.name.clone())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(label) = optional_label {
            // アプリケーションでバリデーションするなら、
            // どうして DB 側に制約を入れないのだろうか？？？
            return Err(RepositoryError::Duplicate(label.id).into());
            // return Ok(label);
        }

        let label = sqlx::query_as::<_, Label>(
            r#"
            INSERT INTO labels (name)
            VALUES ( $1 )
            RETURNING *
            "#,
        )
        .bind(payload.name)
        .fetch_one(&self.pool)
        .await?;

        Ok(label)
    }

    async fn update(&self, id: i32, payload: UpdateLabel) -> anyhow::Result<Label> {
        let old_label = self.find(id).await?;
        let updated_one = sqlx::query_as::<_, Label>(
            r#"
            UPDATE labels SET name=$1
            WHERE id=$2
            RETURNING *
            "#,
        )
        .bind(payload.name.unwrap_or(old_label.name))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated_one)
    }

    async fn find(&self, id: i32) -> anyhow::Result<Label> {
        let label = sqlx::query_as::<_, Label>(
            r#"
                SELECT labels.*
                FROM labels
                WHERE labels.id=$1
                "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        Ok(label.clone())
    }

    async fn find_by_user(&self, user_id: i32) -> anyhow::Result<Vec<Label>> {
        let labels = sqlx::query_as::<_, Label>(
            r#"
                SELECT labels.*
                FROM labels
                WHERE labels.user_id=$1
                "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(user_id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        Ok(labels)
    }

    async fn all(&self) -> anyhow::Result<Vec<Label>> {
        let labels = sqlx::query_as::<_, Label>(
            r#"
            SELECT id, name FROM labels
            ORDER BY id ASC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(labels)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM labels WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "database-test")]
mod test {
    use super::*;
    use dotenv::dotenv;
    use sqlx::PgPool;
    use std::env;

    #[tokio::test]
    async fn crud_scenario() {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("undefined env: [DATABASE_URL]");
        let pool = PgPool::connect(&database_url)
            .await
            .expect(&format!("cannot connect database: [{}]", database_url));
        let repo = LabelRepositoryForDb::new(pool.clone());
        let label_text = "test_label";

        // create
        // name が unique 制約である場合、DB クリアを毎回やらないと成立しない
        let label = repo
            .create(CreateLabel {
                name: label_text.to_string(),
            })
            .await
            .expect("[create] returned Err");
        assert_eq!(label.name, label_text);

        // all
        // let labels = repo.all()
        //     .await
        //     .expect("[all] returned Err");
        // // 連番なので、最後に作ったデータが create の結果と一致しているはずの想定
        // let label = labels.last().unwrap();
        // // assert!(labels.len() == 1); // DB クリアする前提がないので今はこれが安定して成立しない
        // assert_eq!(label.name, label_text);

        // delete
        let _ = repo.delete(label.id).await.expect("[delete] returned Err");
        // let labels = repo.all().await.expect("[all] returned Err");
        // 他 (Label) のテストが途中で失敗するなど、Label が残っている初期状態で
        // このテストが起動してしまうと、次のアサーションは失敗する
        // assert_eq!(labels.len(), 0);
    }
}

#[cfg(test)]
pub mod test_utils {
    use crate::repositories::label::CreateLabel;
    use axum::async_trait;
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    };

    use super::*;

    impl Label {
        pub fn new(id: i32, name: String) -> Self {
            Self { id, name }
        }
    }

    #[cfg(test)]
    impl CreateLabel {
        // pub fn new(name: String) -> Self {
        //     Self { name: name }
        // }
    }

    type LabelDatas = HashMap<i32, Label>;

    #[derive(Debug, Clone)]
    pub struct LabelRepositoryForMemory {
        store: Arc<RwLock<LabelDatas>>,
    }

    impl LabelRepositoryForMemory {
        pub fn new() -> Self {
            LabelRepositoryForMemory {
                store: Arc::default(),
            }
        }

        fn write_store_ref(&self) -> RwLockWriteGuard<LabelDatas> {
            self.store.write().unwrap()
        }

        fn read_store_ref(&self) -> RwLockReadGuard<LabelDatas> {
            self.store.read().unwrap()
        }
    }

    #[async_trait]
    impl LabelRepository for LabelRepositoryForMemory {
        async fn create(&self, payload: CreateLabel) -> anyhow::Result<Label> {
            let mut store = self.write_store_ref();
            let id = (store.len() + 1) as i32;
            let label = Label::new(id, payload.name.clone());
            store.insert(id, label.clone());
            Ok(label)
        }

        async fn find(&self, id: i32) -> anyhow::Result<Label> {
            let store = self.read_store_ref();
            let label = store.get(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(label.clone())
        }

        async fn find_by_user(&self, _user_id: i32) -> anyhow::Result<Vec<Label>> {
            let labels: Vec<Label> = self
                .read_store_ref()
                .values()
                // .filter(|label| label.user_id == user_id)
                .cloned()
                .collect();
            Ok(labels)
        }

        async fn all(&self) -> anyhow::Result<Vec<Label>> {
            let store = self.read_store_ref();
            let labels = Vec::from_iter(store.values().map(|label| label.clone()));
            Ok(labels)
        }

        async fn update(&self, id: i32, payload: UpdateLabel) -> anyhow::Result<Label> {
            let mut store = self.write_store_ref();
            let label = store.get_mut(&id).ok_or(RepositoryError::NotFound(id))?;
            if let Some(name) = payload.name {
                label.name = name;
            }
            Ok(label.clone())
        }

        async fn delete(&self, id: i32) -> anyhow::Result<()> {
            let mut store = self.write_store_ref();
            store.remove(&id).ok_or(RepositoryError::NotFound(id))?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[tokio::test]
        async fn label_crud_scenario() {
            let name = "label name".to_string();
            let id = 1;
            let expected = Label::new(id, name.clone());

            let repo = LabelRepositoryForMemory::new();

            // create
            let label = repo
                .create(CreateLabel { name: name })
                .await
                .expect("failed create label");
            assert_eq!(expected, label);

            // all
            let labels = repo.all().await.expect("failed get all labels");
            assert_eq!(vec![label], labels);

            // delete
            repo.delete(id).await.expect("failed delete label");
            let labels = repo.all().await.expect("failed get all labels");
            assert_eq!(labels.len(), 0);
        }
    }
}
