use actix_web::{cookie::time::error, get, App, HttpResponse, HttpServer, ResponseError, web::{self, Data}, http::header, post};
use thiserror::Error;
use askama::Template;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Deserialize;

// POSTメソッドのADDとDELETEを作成
// まずは構造体
#[derive(Deserialize)]
struct AddParams {
    text: String,
}

#[derive(Deserialize)]
struct DeleteParams {
    id:u32,
}

// 実際の動作を実装
#[post("/add")]
async fn add_todo(
    params: web::Form<AddParams>,
    db: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<HttpResponse, MyError> {
    let conn = db.get()?;
    conn.execute("INSERT INTO todo (text) VALUES (?)", &[&params.text])?;
    Ok(HttpResponse::SeeOther().header(header::LOCATION, "/").finish())
}

#[post("/delete")]
async fn delete_todo(
    params: web::Form<DeleteParams>,
    db: web::Data<Pool<SqliteConnectionManager>>,
) -> Result<HttpResponse, MyError> {
    let conn = db.get()?;
    conn.execute("DELETE FROM todo WHERE id=?", &[&params.id])?;
    Ok(HttpResponse::SeeOther().header(header::LOCATION, "/").finish())
}

// TODOリストの構造体
struct TodoEntry {
    id:u32,
    text:String,
}

// HTML用の構造体
#[derive(Template)]
#[template(path="index.html")]
struct IndexTemplate {
    entries:Vec<TodoEntry>,
}


// エラーをまとめるenumを定義
// actix_web::ResponseErrorとして使うためにderiveマクロでDebugを付与する必要がある
#[derive(Error, Debug)]
enum MyError {
    #[error("Failed to render HTML")]
    askamaError(#[from] askama::Error),

    #[error("Failed to get connection")]
    ConnectionPoolError(#[from] r2d2::Error),

    #[error("Failed SQL execution")]
    SQLiteError(#[from] rusqlite::Error),
}

// actix_web::ResponseError を MyError に実装します
// 今回は新たな実装なし
impl ResponseError for MyError {

}

// GETメソッドを作成。データベースからTODOリストを持ってくる
#[get("/")]
async fn index(db: web::Data<Pool<SqliteConnectionManager>>) -> Result<HttpResponse, MyError> {
    let conn = db.get()?;
    // SQL文をPrepared Statementに変換
    let mut statement = conn.prepare("SELECT id, text FROM todo")?;

    // Prepared StatementとなっているSQL文を実行し、結果をTOdoEntryに変換する
    let rows = statement.query_map(params![], |row|{
        let id = row.get(0)?;
        let text = row.get(1)?;
        Ok(TodoEntry{ id, text })
    })?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    // HTMLを呼び出す
    let html = IndexTemplate {entries};
    // 描画
    let response_body = html.render()?;
    // HttpResponse::OK() はステータスコード200を持つHttpResponseBuilder という構造体を返します
    // HttpResponseBuilder のbody()という関数にレスポンスのボディを渡すとHttpResponseが返ってきます
    // 戻り値の方がResultなのでOK で包みます
    Ok(HttpResponse::Ok().content_type("text/html").body(response_body))
}

#[actix_rt::main]
async fn main() -> Result<(), actix_web::Error> {
    // データベースの初期化
    let manager = SqliteConnectionManager::file("todo.db");
    let pool = Pool::new(manager).expect("Failed to initialize the connection pool.");
    let conn = pool.get().expect("Failed to get the cconnection from the pool");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS todo (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL
        )", 
        params![],
    ).expect("Failed to create a table `todo`.");
    // ここでコネクションプールを渡す
    HttpServer::new(move || {
        App::new()
            .service(index)
            .service(add_todo)
            .service(delete_todo)
            .data(pool.clone())
    })
        .bind("0.0.0.0:8080")?
        .run()
        .await?;
    Ok(())
}
