use axum::{
    extract::{Extension, Path},
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use crate::repositories::label::{
    LabelRepository,
    CreateLabel,
    UpdateLabel,
};
use super::ValidatedJson;

pub async fn create_label<T: LabelRepository>(
    ValidatedJson(payload): ValidatedJson<CreateLabel>,
    Extension(repo): Extension<Arc<T>>,
) -> Result<impl IntoResponse, StatusCode> {
    let label = repo
        .create(payload)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;

    Ok((StatusCode::CREATED, Json(label)))
}

pub async fn find_label<T: LabelRepository>(
    Path(id): Path<i32>,
    Extension(repo): Extension<Arc<T>>,
) -> Result<impl IntoResponse, StatusCode> {
    let label = repo.find(id).await.or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::OK, Json(label)))
}

pub async fn find_by_user<T: LabelRepository>(
    Path(user_id): Path<i32>,
    Extension(repo): Extension<Arc<T>>,
) -> Result<impl IntoResponse, StatusCode> {
    let labels = repo.find_by_user(user_id).await.or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::OK, Json(labels)))
}

pub async fn all_label<T: LabelRepository>(
    Extension(repo): Extension<Arc<T>>,
) -> Result<impl IntoResponse, StatusCode> {
    let labels = repo.all().await.unwrap();
    Ok((StatusCode::OK, Json(labels)))
}

pub async fn update_label<T: LabelRepository>(
    Path(id): Path<i32>,
    ValidatedJson(payload): ValidatedJson<UpdateLabel>,
    Extension(repo): Extension<Arc<T>>,
) -> Result<impl IntoResponse, StatusCode> {
    let label = repo
        .update(id, payload)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    Ok((StatusCode::CREATED, Json(label)))
}

pub async fn delete_label<T: LabelRepository>(
    Path(id): Path<i32>,
    Extension(repo): Extension<Arc<T>>,
) -> impl IntoResponse {
    repo.delete(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}