mod handlers;
mod repositories;

use crate::repositories::{
    label::{LabelRepository, LabelRepositoryForDb},
    todo::{TodoRepository, TodoRepositoryForDb},
};
use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use handlers::{
    label::{all_label, create_label, delete_label, find_by_user, find_label, update_label},
    todo::{all_todo, create_todo, delete_todo, find_todo, update_todo},
};
use hyper::header::CONTENT_TYPE;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::{env, sync::Arc};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

#[tokio::main]
async fn main() {
    // read .env file
    let app_env = env::var("APP_ENV").unwrap_or("local".to_string());
    dotenv::from_filename(".config/.env.".to_string() + app_env.as_str()).ok();

    // set log level
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
    dotenv().ok();

    // set database
    // let repo = TodoRepositoryForMemory::new();
    let database_url = env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");
    tracing::debug!("startconnect database...");
    let pool = PgPool::connect(database_url.as_str())
        .await
        .expect(&format!("cannot connect to database: [{}]", database_url));

    // build app
    let app = create_app(
        TodoRepositoryForDb::new(pool.clone()),
        LabelRepositoryForDb::new(pool.clone()),
    );
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    tracing::debug!("listening on {}", addr);

    // serve
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// create app with repositories. return Router
fn create_app<Todo: TodoRepository, Label: LabelRepository>(
    todo_repository: Todo,
    label_repository: Label,
) -> Router {
    let allow_origin_url: std::string::String =
        env::var("ALLOW_ORIGIN_URL").expect("ALLOW_ORIGIN_URL must be set");

    Router::new()
        .route("/", get(root))
        .route("/todos", post(create_todo::<Todo>).get(all_todo::<Todo>))
        .route(
            "/todos/:id",
            get(find_todo::<Todo>)
                .delete(delete_todo::<Todo>)
                .patch(update_todo::<Todo>),
        )
        .route(
            "/labels",
            post(create_label::<Label>).get(all_label::<Label>),
        )
        .route("/labels/:id", delete(delete_label::<Label>))
        .route(
            "/labels/:id",
            get(find_label::<Label>).patch(update_label::<Label>),
        )
        .route("/labels/user/:user_id", get(find_by_user::<Label>))
        .layer(Extension(Arc::new(todo_repository)))
        .layer(Extension(Arc::new(label_repository)))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::exact(allow_origin_url.parse().unwrap()))
                .allow_methods(Any)
                .allow_headers(vec![CONTENT_TYPE]),
        )
}

async fn root() -> &'static str {
    "hello world"
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::repositories::label::test_utils::LabelRepositoryForMemory;
    use crate::repositories::todo::{test_utils::TodoRepositoryForMemory, CreateTodo, Todo};
    use axum::response::Response;
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
    };
    use tower::ServiceExt;

    fn build_todo_req_with_json(path: &str, method: Method, json_body: String) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.to_string())
            .body(Body::from(json_body))
            .unwrap()
    }

    fn build_todo_req_with_empty(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap()
    }

    async fn res_to_todo(res: Response) -> Todo {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: Todo = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo instance. body: {}", body));
        todo
    }

    #[tokio::test]
    async fn should_return_hello_world() {
        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let router = create_app(todo_repo, label_repo);
        let res = router.oneshot(req).await.unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "hello world");
    }

    #[tokio::test]
    async fn should_created_todo() {
        let expected = Todo::new(1, "should_return_created_todo".to_string());

        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        let req = build_todo_req_with_json(
            "/todos",
            Method::POST,
            r#"{
                "text": "should_return_created_todo",
                "labels": []

            }"#
            .to_string(),
        );
        let res = create_app(todo_repo, label_repo)
            .oneshot(req)
            .await
            .expect("failed create todo");

        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_find_todo() {
        let expected = Todo::new(1, "should_find_todo".to_string());

        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        todo_repo
            .create(CreateTodo::new("should_find_todo".to_string(), vec![]))
            .await
            .expect("cannot create todo");
        let req = build_todo_req_with_empty(Method::GET, "/todos/1");
        let res = create_app(todo_repo, label_repo)
            .oneshot(req)
            .await
            .unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_get_all_todos() {
        let expected = Todo::new(1, "should_get_all_todos".to_string());

        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        todo_repo
            .create(CreateTodo::new("should_get_all_todos".to_string(), vec![]))
            .await
            .expect("cannot create todo");
        let req = build_todo_req_with_empty(Method::GET, "/todos");
        let res = create_app(todo_repo, label_repo)
            .oneshot(req)
            .await
            .unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: Vec<Todo> = serde_json::from_str(&body)
            .expect(&format!("cannot convert Todo instance. body: {:?}", body));
        assert_eq!(vec![expected], todo);
    }

    #[tokio::test]
    async fn should_update_todo() {
        let expected = Todo::new(1, "should_update_todo".to_string());

        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        todo_repo
            .create(CreateTodo::new("before_update_todo".to_string(), vec![]))
            .await
            .expect("cannot create todo");
        let req = build_todo_req_with_json(
            "/todos/1",
            Method::PATCH,
            r#"{
                "text": "should_update_todo",
                "completed": false
            }"#
            .to_string(),
        );
        let res = create_app(todo_repo, label_repo)
            .oneshot(req)
            .await
            .unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_delete_todo() {
        let todo_repo = TodoRepositoryForMemory::new();
        let label_repo = LabelRepositoryForMemory::new();
        todo_repo
            .create(CreateTodo::new("should_delete_todo".to_string(), vec![]))
            .await
            .expect("cannot create todo");
        let req = build_todo_req_with_empty(Method::DELETE, "/todos/1");
        let res = create_app(todo_repo, label_repo)
            .oneshot(req)
            .await
            .unwrap();
        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }
}
